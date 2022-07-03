use std::{fmt::Debug, time::Instant};

use image::Rgb;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::{
    history::History,
    make_tool_transform,
    tools::core::Manipulate,
    types::ViewImage,
    util::{self, Shape, BB},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};

const MIN_ZOOM: u32 = 2;

fn make_zoom_on_release(
    mouse_pos_start: (usize, usize),
    mouse_pos_release: (usize, usize),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> Option<BB> {
    let prs_orig =
        util::mouse_pos_to_orig_pos(Some(mouse_pos_start), shape_orig, shape_win, zoom_box);
    let rel_orig =
        util::mouse_pos_to_orig_pos(Some(mouse_pos_release), shape_orig, shape_win, zoom_box);

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
    let press_orig = util::mouse_pos_to_orig_pos(Some(m_press), shape_orig, shape_win, &zoom_box);
    let held_orig = util::mouse_pos_to_orig_pos(Some(m_held), shape_orig, shape_win, &zoom_box);
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

fn draw_bx_on_view(im_prev_view: &ViewImage, world: &mut World, start_time: Instant, draw_bx: BB) {
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
            let rgb = *world.im_view().get_pixel(x, y);
            let rgb_prev = im_prev_view.get_pixel(x, y);
            world.set_view_pixel(
                x,
                y,
                Rgb([
                    change(rgb[0], rgb_prev[0]),
                    change(rgb[1], rgb_prev[1]),
                    change(rgb[2], rgb_prev[2]),
                ]),
            );
        }
    }
}

#[derive(Clone, Debug)]
pub struct Zoom {
    mouse_pressed_start_time: Option<Instant>,
    mouse_pressed_start_pos: Option<(usize, usize)>,
    im_prev_view: Option<ViewImage>,
    animation_box: Option<BB>,
}
impl Zoom {

    fn set_mouse_start(&mut self, mp: (usize, usize), im_view: Option<ViewImage>) {
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
        history: History,
    ) -> (World, History) {
        if let (None, Some((m_x, m_y))) = (self.mouse_pressed_start_pos, mouse_pos) {
            self.set_mouse_start((m_x, m_y), Some(world.im_view().clone()));
        }
        (world, history)
    }
    
    fn mouse_released(
        &mut self,
        btn: usize,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if btn == LEFT_BTN {
            let shape_orig = world.shape_orig();
            let bx = if let (Some(mps), Some(mr)) = (self.mouse_pressed_start_pos, mouse_pos) {
                make_zoom_on_release(mps, mr, shape_orig, shape_win, &self.animation_box)
            } else {
                None
            };
            world.set_zoom_box(bx, shape_win);
            self.unset_mouse_start();
            (world, history)
        } else if btn == RIGHT_BTN {
            self.unset_mouse_start();
            (world, history)
        } else {
            (world, history)
        }
    }

    fn mouse_held(
        &mut self,
        btn: usize,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if btn == RIGHT_BTN {
            if let (Some(mps), Some(mp)) = (self.mouse_pressed_start_pos, mouse_pos) {
                world.set_zoom_box(
                    move_zoom_box(
                        mps,
                        mp,
                        world.ims_raw().shape(),
                        shape_win,
                        *world.zoom_box(),
                    ),
                    shape_win,
                );
                match mouse_pos {
                    Some(mp) => {
                        self.set_mouse_start(mp, None);
                    }
                    None => {
                        self.unset_mouse_start();
                    }
                }
            }
            (world, history)
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
                draw_bx_on_view(im_prev_view, &mut world, start_time, draw_bx);
            }
            (world, history)
        } else {
            (world, history)
        }
    }

    fn key_pressed(
        &mut self,
        _key: VirtualKeyCode,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        world.set_zoom_box(None, shape_win);
        (world, history)
    }

}
impl Manipulate for Zoom {
    fn new() -> Zoom {
        Zoom {
            animation_box: None,
            mouse_pressed_start_time: None,
            mouse_pressed_start_pos: None,
            im_prev_view: None,
        }
    }
    fn events_tf(
        &mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
    ) -> (World, History) {
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [mouse_pressed, mouse_released, mouse_held],
            [VirtualKeyCode::Back, VirtualKeyCode::R]
        )
    }
}

#[cfg(test)]
use {crate::result::RvResult, image::DynamicImage};
#[cfg(test)]
fn make_shape_win(shape_orig: Shape, zoom_box: Option<BB>) -> Shape {
    match zoom_box {
        None => shape_orig,
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
            make_zoom_on_release(mpp, mpr, shape_orig, shape_win, &zoom_box),
            expected
        );
    }

    test((0, 0), (10, 10), None, mk_z(0, 0, 10, 10));
    test((0, 0), (100, 10), None, None);
    test((13, 7), (33, 17), None, mk_z(13, 7, 20, 10));
    test((5, 9), (6, 10), mk_z(24, 36, 33, 55), None);
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
fn test_on_mouse_pressed() -> RvResult<()> {
    let shape_win = Shape { w: 250, h: 500 };
    let mouse_pos = Some((30, 45));
    let im_orig = DynamicImage::ImageRgb8(ViewImage::new(250, 500));
    let mouse_btn = LEFT_BTN;
    let mut z = Zoom::new();
    let world = World::from_im(im_orig, shape_win);
    let history = History::new();
    let im_view_old = world.im_view().clone();
    let im_orig_old = world.ims_raw().clone();
    let (res, _) = z.mouse_pressed(mouse_btn, shape_win, mouse_pos, world, history);
    assert_eq!(z.im_prev_view.unwrap(), im_view_old);
    assert_eq!(*res.im_view(), im_view_old);
    assert_eq!(*res.ims_raw(), im_orig_old);
    assert_eq!(z.mouse_pressed_start_pos, mouse_pos);
    Ok(())
}

#[test]
fn test_on_mouse_released() -> RvResult<()> {
    let shape_win = Shape { w: 250, h: 500 };
    let mouse_pos = Some((30, 70));
    let im_orig = DynamicImage::ImageRgb8(ViewImage::new(250, 500));
    let mouse_btn = LEFT_BTN;
    let mut z = Zoom::new();
    let world = World::from_im(im_orig, shape_win);
    let history = History::new();
    z.set_mouse_start((40, 80), Some(world.im_view().clone()));

    let (world, _) = z.mouse_released(mouse_btn, shape_win, mouse_pos, world, history);
    assert_eq!(Shape::new(250, 250), Shape::from_im(world.im_view()));
    assert_eq!(
        *world.zoom_box(),
        Some(BB {
            x: 30,
            y: 70,
            w: 10,
            h: 10
        })
    );
    assert_eq!(z.im_prev_view, None);
    assert_eq!(z.mouse_pressed_start_pos, None);
    Ok(())
}

#[test]
fn test_on_mouse_held() {}
