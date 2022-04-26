use std::{fmt::Debug, time::Instant};

use image::{
    imageops::{self, FilterType},
    GenericImageView, Rgb,
};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::{
    make_event_handler_if_elses,
    util::{mouse_pos_to_orig_pos, shape_from_im, shape_scaled, shape_unscaled, Shape, BB},
    world::World,
    ImageType, LEFT_BTN, RIGHT_BTN,
};

use super::Tool;

const MIN_ZOOM: u32 = 2;

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

pub fn scale_to_win(
    im_orig: &ImageType,
    zoom_box: Option<BB>,
    w_win: u32,
    h_win: u32,
) -> ImageType {
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

fn draw_bx_on_view(
    im_prev_view: &ImageType,
    im_view: &mut ImageType,
    start_time: Instant,
    draw_bx: BB,
) {
    let max_offset = 100;
    let offset = (start_time.elapsed().as_millis() as f64 / 250.0).min(max_offset as f64) as u8;
    let change = |v, v_prev| {
        let upper_bound = if v_prev >= 255 - max_offset as u8 {
            255
        } else {
            v_prev + max_offset as u8
        };
        let upper_bound = upper_bound.max(offset);
        if v < upper_bound - offset {
            v + offset
        } else {
            upper_bound
        }
    };
    for y in draw_bx.y..(draw_bx.y + draw_bx.h) {
        for x in draw_bx.x..(draw_bx.x + draw_bx.w) {
            let rgb = im_view.get_pixel_mut(x, y);
            let rgb_prev = im_prev_view.get_pixel(x, y);
            *rgb = Rgb([
                change(rgb[0], rgb_prev[0]),
                change(rgb[1], rgb_prev[1]),
                change(rgb[2], rgb_prev[2]),
            ]);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Zoom {
    bx: Option<BB>,
    mouse_pressed_start_time: Option<Instant>,
    mouse_pressed_start_pos: Option<(usize, usize)>,
    im_prev_view: Option<ImageType>,
}
impl Zoom {
    fn set_mouse_start(&mut self, mp: (usize, usize), im_view: Option<ImageType>) {
        self.mouse_pressed_start_pos = Some(mp);
        self.mouse_pressed_start_time = Some(Instant::now());
        self.im_prev_view = im_view;
    }
    fn unset_mouse_start(&mut self) {
        self.mouse_pressed_start_pos = None;
        self.mouse_pressed_start_time = None;
        self.im_prev_view = None;
    }
    fn mouse_pressed(
        &mut self,
        _btn: usize,
        _shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        world: World,
    ) -> World {
        if let (None, Some((m_x, m_y))) = (self.mouse_pressed_start_pos, mouse_pos) {
            self.set_mouse_start((m_x, m_y), Some(world.im_view().clone()));
        }
        world
    }
    fn mouse_released(
        &mut self,
        btn: usize,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
    ) -> World {
        if btn == LEFT_BTN {
            let shape_orig = world.shape_orig();
            let im_view = if let (Some(mps), Some(mr)) = (self.mouse_pressed_start_pos, mouse_pos) {
                let bx = make_zoom_on_release(mps, mr, shape_orig, shape_win, self.bx);
                if let Some(bx_) = bx {
                    self.bx = Some(bx_);
                }
                scale_to_win(world.im_orig(), self.bx, shape_win.w, shape_win.h)
            } else {
                scale_to_win(world.im_orig(), self.bx, shape_win.w, shape_win.h)
            };
            self.unset_mouse_start();
            *world.im_view_mut() = im_view;
            world
        } else if btn == RIGHT_BTN {
            self.unset_mouse_start();
            world
        } else {
            world
        }
    }
    fn mouse_held(
        &mut self,
        btn: usize,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
    ) -> World {
        if btn == RIGHT_BTN {
            if let (Some(mps), Some(mp)) = (self.mouse_pressed_start_pos, mouse_pos) {
                self.bx =
                    move_zoom_box(mps, mp, shape_from_im(world.im_orig()), shape_win, self.bx);
                let im_view = scale_to_win(world.im_orig(), self.bx, shape_win.w, shape_win.h);
                match mouse_pos {
                    Some(mp) => {
                        self.set_mouse_start(mp, None);
                    }
                    None => {
                        self.unset_mouse_start();
                    }
                }
                *world.im_view_mut() = im_view;
            }
            world
        } else if btn == LEFT_BTN {
            if let (Some((mps_x, mps_y)), Some((m_x, m_y)), Some(start_time), Some(im_prev_view)) = (
                self.mouse_pressed_start_pos,
                mouse_pos,
                self.mouse_pressed_start_time,
                &self.im_prev_view,
            ) {
                let x_min = mps_x.min(m_x);
                let y_min = mps_y.min(m_y);
                let x_max = mps_x.max(m_x);
                let y_max = mps_y.max(m_y);
                let draw_bx = BB {
                    x: x_min as u32,
                    y: y_min as u32,
                    w: (x_max - x_min) as u32,
                    h: (y_max - y_min) as u32,
                };
                draw_bx_on_view(im_prev_view, world.im_view_mut(), start_time, draw_bx);
            }
            world
        } else {
            world
        }
    }
    fn key_pressed(
        &mut self,
        key: VirtualKeyCode,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
    ) -> World {
        if key == VirtualKeyCode::Back {
            self.bx = None;
            *world.im_view_mut() = scale_to_win(world.im_orig(), self.bx, shape_win.w, shape_win.h);
        }
        world
    }
}
impl Tool for Zoom {
    fn new() -> Zoom {
        Zoom {
            bx: None,
            mouse_pressed_start_time: None,
            mouse_pressed_start_pos: None,
            im_prev_view: None,
        }
    }
    fn old_to_new(self) -> Self {
        // zoom keeps everything identical when transforming from old to new
        self
    }

    fn events_transform<'a>(
        &'a mut self,
        input_event: &'a WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
    ) -> Box::<dyn 'a + FnMut(World) -> World> {
        make_event_handler_if_elses!(
            self,
            input_event,
            shape_win,
            mouse_pos,
            [mouse_pressed, mouse_released, mouse_held],
            [VirtualKeyCode::Back]
        )
    }

    fn scale_to_shape(&self, world: &mut World, shape: &Shape) -> Option<ImageType> {
        Some(scale_to_win(world.im_orig(), self.bx, shape.w, shape.h))
    }

    fn get_pixel_on_orig(
        &self,
        im_orig: &ImageType,
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
fn test_scale_to_win() {
    let mut im_test = ImageType::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    im_test.put_pixel(10, 10, Rgb([23, 23, 23]));
    let im_scaled = scale_to_win(&im_test, None, 128, 128);
    assert_eq!(im_scaled.get_pixel(0, 0).0, [23, 23, 23]);
    assert_eq!(im_scaled.get_pixel(20, 20).0, [23, 23, 23]);
    assert_eq!(im_scaled.get_pixel(70, 70).0, [0, 0, 0]);
}
#[test]
fn test_on_mouse_pressed() {
    let shape_win = Shape { w: 250, h: 500 };
    let mouse_pos = Some((30, 45));
    let im_orig = ImageType::new(250, 500);
    let mouse_btn = LEFT_BTN;
    let mut z = Zoom::new();
    let world = World::new(im_orig);
    let old_world = world.clone();
    let res = z.mouse_pressed(mouse_btn, shape_win, mouse_pos, world);
    assert_eq!(res, old_world);
    assert_eq!(&z.im_prev_view.unwrap(), old_world.im_view());
    assert_eq!(z.mouse_pressed_start_pos, mouse_pos);
}

#[test]
fn test_on_mouse_released() {
    let shape_win = Shape { w: 250, h: 500 };
    let mouse_pos = Some((30, 45));
    let im_orig = ImageType::new(250, 500);
    let mouse_btn = LEFT_BTN;
    let mut z = Zoom::new();
    let world = World::new(im_orig);
    z.set_mouse_start((40, 80), Some(world.im_view().clone()));

    let res = z.mouse_released(mouse_btn, shape_win, mouse_pos, world);
    let shape_scaled_to_win = shape_scaled(z.bx.unwrap().shape(), shape_win);
    assert_eq!(shape_scaled_to_win, shape_from_im(&res.im_view()));
    assert_eq!(
        z.bx,
        Some(BB {
            x: 30,
            y: 45,
            w: 10,
            h: 35
        })
    );
    assert_eq!(z.im_prev_view, None);
    assert_eq!(z.mouse_pressed_start_pos, None);
}

#[test]
fn test_on_mouse_held() {}
