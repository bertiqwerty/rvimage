use std::fmt::Debug;

use image::{
    imageops::{self, FilterType},
    GenericImageView, ImageBuffer, Rgb,
};
use pixels::Pixels;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::{
    util::{mouse_pos_to_orig_pos, mouse_pos_transform, shape_scaled, shape_unscaled, Shape, BB},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};

use super::Tool;

const MIN_ZOOM: u32 = 10;

fn make_zoom_on_release(
    mouse_pos_start: (usize, usize),
    mouse_pos_end: (usize, usize),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: Option<BB>,
) -> Option<BB> {
    let prs_orig = mouse_pos_to_orig_pos(Some(mouse_pos_start), shape_orig, shape_win, &zoom_box);
    let rel_orig = mouse_pos_to_orig_pos(Some(mouse_pos_end), shape_orig, shape_win, &zoom_box);

    match (prs_orig, rel_orig) {
        (Some((px, py)), Some((rx, ry))) => {
            let x_min = px.min(rx) as u32;
            let y_min = py.min(ry) as u32;
            let x_max = px.max(rx) as u32;
            let y_max = py.max(ry) as u32;

            let w = x_max - x_min;
            let h = y_max - y_min;
            if w >= MIN_ZOOM && h >= MIN_ZOOM {
                Some(BB {
                    x: x_min,
                    y: y_min,
                    w,
                    h,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

fn move_zoom_box(
    m_press: (usize, usize),
    m_held: (usize, usize),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: Option<BB>,
) -> Option<BB> {
    let press_orig = mouse_pos_to_orig_pos(Some(m_press), shape_orig, shape_win, &zoom_box);
    let held_orig = mouse_pos_to_orig_pos(Some(m_held), shape_orig, shape_win, &zoom_box);
    match (press_orig, held_orig, zoom_box) {
        (Some((px, py)), Some((hx, hy)), Some(c)) => {
            let x_shift: i32 = px as i32 - hx as i32;
            let y_shift: i32 = py as i32 - hy as i32;
            let x_new = c.x as i32 + x_shift;
            let y_new = c.y as i32 + y_shift;
            if x_new >= 0
                && y_new >= 0
                && x_new as u32 + c.w < shape_orig.w
                && y_new as u32 + c.h < shape_orig.h
            {
                Some(BB {
                    x: x_new as u32,
                    y: y_new as u32,
                    w: c.w,
                    h: c.h,
                })
            } else {
                zoom_box
            }
        }
        _ => zoom_box,
    }
}

/// Draw the image to the frame buffer.
///
/// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
fn draw(im_view: &ImageBuffer<Rgb<u8>, Vec<u8>>, draw_zoom_box: &Option<BB>, pixels: &mut Pixels) {
    let frame_len = pixels.get_frame().len() as u32;
    if frame_len != im_view.width() * im_view.height() * 4 {
        pixels.resize_buffer(im_view.width(), im_view.height())
    }
    let frame = pixels.get_frame();

    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
        let x = (i % im_view.width() as usize) as u32;
        let y = (i / im_view.width() as usize) as u32;
        let rgb = im_view.get_pixel(x, y).0;
        let rgb_changed = if let Some(dc) = draw_zoom_box {
            let offset = 50;
            let change = |x| if 255 - x > offset { x + offset } else { 255 };
            if x >= dc.x && y >= dc.y && x < dc.x + dc.w && y < dc.y + dc.h {
                [change(rgb[0]), change(rgb[1]), change(rgb[2])]
            } else {
                rgb
            }
        } else {
            rgb
        };
        let rgba = [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff];

        pixel.copy_from_slice(&rgba);
    }
}

pub fn scale_world_to_match_win(
    world: &mut World,
    zoom_box: Option<BB>,
    shape_orig: Shape,
    w_win: u32,
    h_win: u32,
) -> Shape {
    let unscaled = shape_unscaled(&zoom_box, shape_orig);
    let new = shape_scaled(unscaled, Shape { w: w_win, h: h_win });
    match zoom_box {
        Some(c) => {
            world.set_im_view(imageops::resize(
                &*world.im_orig().view(c.x, c.y, c.w, c.h),
                new.w,
                new.h,
                FilterType::Nearest,
            ));
        }
        None => {
            world.set_im_view(imageops::resize(
                world.im_orig(),
                new.w,
                new.h,
                FilterType::Nearest,
            ));
        }
    }
    new
}

#[derive(Clone, Debug)]
pub struct Zoom {
    bx: Option<BB>,
    draw_bx: Option<BB>,
    mouse_pressed_start_pos: Option<(usize, usize)>,
}
impl Zoom {
    fn set_mouse_start(&mut self, mp: (usize, usize)) {
        self.mouse_pressed_start_pos = Some(mp);
    }
    fn unset_mouse_start(&mut self) {
        self.mouse_pressed_start_pos = None;
    }
}
impl Tool for Zoom {
    fn new() -> Zoom {
        println!("new zoom");
        Zoom {
            bx: None,
            draw_bx: None,
            mouse_pressed_start_pos: None,
        }
    }
    fn old_to_new(self) -> Self {
        // zoom keeps everything identical when transforming from old to new
        self
    }

    fn events_transform(
        &mut self,
        input_event: &WinitInputHelper,
        shape_win: Shape,
        pixels: &mut Pixels,
        world: &mut World,
    ) {
        let w_win = shape_win.w;
        let h_win = shape_win.h;
        let bx = self.bx;
        let shape_orig = world.shape_orig();
        let mut scale_world = |w, h| scale_world_to_match_win(world, bx, shape_orig, w, h);

        // zoom
        let mouse_pos = mouse_pos_transform(pixels, input_event.mouse());
        if input_event.mouse_pressed(LEFT_BTN) || input_event.mouse_pressed(RIGHT_BTN) {
            if let (None, Some((m_x, m_y))) = (self.mouse_pressed_start_pos, mouse_pos) {
                self.set_mouse_start((m_x, m_y));
            }
        }
        if input_event.mouse_released(LEFT_BTN) {
            if let (Some(mps), Some(mr)) = (self.mouse_pressed_start_pos, mouse_pos) {
                self.bx = make_zoom_on_release(mps, mr, shape_orig, shape_win, self.bx);
                if self.bx.is_some() {
                    let shape = scale_world(w_win, h_win);
                    pixels.resize_buffer(shape.w, shape.h);
                }
            }
            println!("noonining start");
            self.unset_mouse_start();
            println!("draw box to None");
            self.draw_bx = None;
        }
        // zoom move
        if input_event.mouse_held(RIGHT_BTN) {
            if let (Some(mps), Some(mp)) = (self.mouse_pressed_start_pos, mouse_pos) {
                self.bx = move_zoom_box(mps, mp, shape_orig, shape_win, self.bx);
                scale_world(w_win, h_win);
                match mouse_pos {
                    Some(mp) => {
                        self.set_mouse_start(mp);
                    }
                    None => {
                        self.unset_mouse_start();
                    }
                }
            }
        // define zoom
        } else if input_event.mouse_held(LEFT_BTN) {
            if let (Some((mps_x, mps_y)), Some((m_x, m_y))) =
                (self.mouse_pressed_start_pos, mouse_pos)
            {
                let x_min = mps_x.min(m_x);
                let y_min = mps_y.min(m_y);
                let x_max = mps_x.max(m_x);
                let y_max = mps_y.max(m_y);
                self.draw_bx = Some(BB {
                    x: x_min as u32,
                    y: y_min as u32,
                    w: (x_max - x_min) as u32,
                    h: (y_max - y_min) as u32,
                });
                println!("set drawbox to {:?}", self.draw_bx);
            }
            println!("mstartpos {:?}", self.mouse_pressed_start_pos);
        }
        if input_event.mouse_released(RIGHT_BTN) {
            println!("noonining start");
            self.unset_mouse_start();
        }
        // unzoom
        if input_event.key_pressed(VirtualKeyCode::Back) {
            self.bx = None;
            pixels.resize_buffer(shape_win.w, shape_win.h);
        }
    }
    fn draw(&self, world: &World, pixels: &mut Pixels) {
        draw(world.im_view(), &self.draw_bx, pixels);
    }
    fn scale_to_shape(&self, world: &mut World, shape: &Shape) -> Option<Shape> {
        Some(scale_world_to_match_win(
            world,
            self.bx,
            world.shape_orig(),
            shape.w,
            shape.h,
        ))
    }

    fn get_pixel_on_orig(
        &self,
        world: &World,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
    ) -> Option<(u32, u32, [u8; 3])> {
        let pos = mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, &self.bx);
        pos.map(|(x, y)| (x, y, world.im_orig().get_pixel(x, y).0))
    }
}
