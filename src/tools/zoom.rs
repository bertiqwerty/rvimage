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
    mouse_pos_release: (usize, usize),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: Option<BB>,
) -> Option<BB> {
    let prs_orig = mouse_pos_to_orig_pos(Some(mouse_pos_start), shape_orig, shape_win, &zoom_box);
    let rel_orig = mouse_pos_to_orig_pos(Some(mouse_pos_release), shape_orig, shape_win, &zoom_box);

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
fn pixels_rgba_at(
    i: usize,
    im_view: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    draw_zoom_box: &Option<BB>,
) -> [u8; 4] {
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
    [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff]
}

pub fn scale_to_win(
    im_orig: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    zoom_box: Option<BB>,
    w_win: u32,
    h_win: u32,
) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let shape_orig = Shape {
        w: im_orig.width(),
        h: im_orig.height(),
    };
    let unscaled = shape_unscaled(&zoom_box, shape_orig);
    let new = shape_scaled(unscaled, Shape { w: w_win, h: h_win });
    match zoom_box {
        Some(c) => imageops::resize(
            &*im_orig.view(c.x, c.y, c.w, c.h),
            new.w,
            new.h,
            FilterType::Nearest,
        ),
        None => imageops::resize(im_orig, new.w, new.h, FilterType::Nearest),
    }
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
                    let im_view = scale_to_win(world.im_orig(), bx, w_win, h_win);
                    world.set_im_view(im_view);
                    pixels.resize_buffer(world.im_view().width(), world.im_view().height());
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
                let im_view = scale_to_win(world.im_orig(), bx, w_win, h_win);
                world.set_im_view(im_view);
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
        let frame_len = pixels.get_frame().len() as u32;
        let w_view = world.im_view().width();
        let h_view = world.im_view().height();
        if frame_len != w_view * h_view * 4 {
            pixels.resize_buffer(w_view, h_view);
        }
        let frame = pixels.get_frame();

        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let rgba = pixels_rgba_at(i, world.im_view(), &self.draw_bx);
            pixel.copy_from_slice(&rgba);
        }
    }
    fn scale_to_shape(&self, world: &mut World, shape: &Shape) -> Option<Shape> {
        let im_view = scale_to_win(world.im_orig(), self.bx, shape.w, shape.h);
        let shape = Shape {
            w: im_view.width(),
            h: im_view.height(),
        };
        world.set_im_view(im_view);
        Some(shape)
    }

    fn get_pixel_on_orig(
        &self,
        im_orig: &ImageBuffer<Rgb<u8>, Vec<u8>>,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
    ) -> Option<(u32, u32, [u8; 3])> {
        let shape_orig = Shape {
            w: im_orig.width(),
            h: im_orig.height(),
        };
        let pos = mouse_pos_to_orig_pos(mouse_pos, shape_orig, shape_win, &self.bx);
        pos.map(|(x, y)| (x, y, im_orig.get_pixel(x, y).0))
    }
}
#[cfg(test)]
use crate::result::RvResult;
#[cfg(test)]
fn make_shape_win(shape_orig: Shape, zoom_box: Option<BB>) -> Shape {
    match zoom_box {
        None => Shape {
            w: shape_orig.w,
            h: shape_orig.h,
        },
        Some(zb) => zb.shape(),
    }
}
#[cfg(test)]
fn mk_z(x: u32, y: u32, w: u32, h: u32) -> Option<BB> {
    Some(BB { x, y, w, h })
}
#[test]
fn test_make_zoom() -> RvResult<()> {
    fn test(mpp: (usize, usize), mpr: (usize, usize), zoom_box: Option<BB>, expected: Option<BB>) {
        let shape_orig = Shape { w: 80, h: 80 };
        let shape_win = make_shape_win(shape_orig, zoom_box);
        assert_eq!(
            make_zoom_on_release(mpp, mpr, shape_orig, shape_win, zoom_box),
            expected
        );
    }

    test((0, 0), (10, 10), None, mk_z(0, 0, 10, 10));
    test((0, 0), (100, 10), None, None);
    test((13, 7), (33, 17), None, mk_z(13, 7, 20, 10));
    test((5, 9), (10, 19), mk_z(24, 36, 33, 55), None);
    test((5, 9), (17, 19), mk_z(24, 36, 33, 55), mk_z(29, 45, 12, 10));

    Ok(())
}
#[test]
fn test_move_zoom() -> RvResult<()> {
    fn test(mpp: (usize, usize), mph: (usize, usize), zoom_box: Option<BB>, expected: Option<BB>) {
        let shape_orig = Shape { w: 80, h: 80 };
        let shape_win = make_shape_win(shape_orig, zoom_box);
        assert_eq!(
            move_zoom_box(mpp, mph, shape_orig, shape_win, zoom_box),
            expected
        );
    }
    test((4, 4), (12, 8), mk_z(12, 16, 40, 40), mk_z(4, 12, 40, 40));
    Ok(())
}
#[test]
fn test_rgba() {
    let mut im_test = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(0, &im_test, &None), [23, 23, 23, 255]);
    im_test.put_pixel(0, 1, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(64, &im_test, &None), [23, 23, 23, 255]);
    im_test.put_pixel(7, 11, Rgb([23, 23, 23]));
    assert_eq!(
        pixels_rgba_at(11 * 64 + 7, &im_test, &None),
        [23, 23, 23, 255]
    );
    assert_eq!(
        pixels_rgba_at(11 * 64 + 7, &im_test, &mk_z(5, 10, 24, 24)),
        [73, 73, 73, 255]
    );
}
#[test]
fn test_scale_to_win() {
    let mut im_test = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    im_test.put_pixel(10, 10, Rgb([23, 23, 23]));
    let im_scaled = scale_to_win(&im_test, None, 128, 128);
    assert_eq!(im_scaled.get_pixel(0, 0).0, [23, 23, 23]);
    assert_eq!(im_scaled.get_pixel(20, 20).0, [23, 23, 23]);
    assert_eq!(im_scaled.get_pixel(70, 70).0, [0, 0, 0]);
}
