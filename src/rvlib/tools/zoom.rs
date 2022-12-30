use image::Rgb;
use std::fmt::Debug;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::{
    domain::{self, BbViewPointIterator, Shape, BB},
    history::History,
    image_util::{self, to_u32},
    make_tool_transform,
    tools::core::Manipulate,
    types::ViewImage,
    world::World,
    LEFT_BTN, RIGHT_BTN,
};

use super::core::Mover;
const MIN_ZOOM: u32 = 2;

fn draw_zoombox_on_view(im: ViewImage, zoombox: BB, color: &Rgb<u8>) -> ViewImage {
    let offset = Rgb([color[0] / 5, color[1] / 5, color[2] / 5]);
    let f = |rgb: &Rgb<u8>| {
        Rgb([
            image_util::clipped_add(offset[0], rgb[0], 255),
            image_util::clipped_add(offset[1], rgb[1], 255),
            image_util::clipped_add(offset[2], rgb[2], 255),
        ])
    };
    let corners = zoombox.corners();
    let inner_points = BbViewPointIterator::from_bb(zoombox);
    image_util::draw_on_image(im, corners, inner_points, color, f)
}
pub fn move_zoom_box(
    mut mover: Mover,
    mut world: World,
    mouse_pos: Option<(usize, usize)>,
    shape_win: Shape,
) -> (Mover, World) {
    let shape_orig = world.data.shape();
    let zoom_box = *world.zoom_box();
    let f_move = |mpso, mpo| follow_zoom_box(mpso, mpo, shape_orig, zoom_box);
    let opt_opt_zoom_box =
        mover.move_mouse_held(f_move, mouse_pos, shape_win, shape_orig, &zoom_box);
    if let Some(zoom_box) = opt_opt_zoom_box {
        world.set_zoom_box(zoom_box, shape_win);
    }
    (mover, world)
}

fn make_zoom_on_release(
    mouse_pos_start: (usize, usize),
    mouse_pos_release: (usize, usize),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> Option<BB> {
    let prs_orig =
        domain::mouse_pos_to_orig_pos(Some(mouse_pos_start), shape_orig, shape_win, zoom_box);
    let rel_orig =
        domain::mouse_pos_to_orig_pos(Some(mouse_pos_release), shape_orig, shape_win, zoom_box);

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

fn follow_zoom_box(
    mpso: (u32, u32),
    mpo: (u32, u32),
    shape_orig: Shape,
    zoom_box: Option<BB>,
) -> Option<BB> {
    match zoom_box {
        Some(zb) => match zb.follow_movement(mpo, mpso, shape_orig) {
            Some(zb) => Some(zb),
            None => Some(zb),
        },
        _ => zoom_box,
    }
}

#[derive(Clone, Debug)]
pub struct Zoom {
    mouse_pressed_start_pos: Option<(usize, usize)>,
    mover: Mover,
    initial_view: Option<ViewImage>,
}
impl Zoom {
    fn set_mouse_start_zoom(&mut self, mp: (usize, usize), im_view_initial: Option<ViewImage>) {
        self.mouse_pressed_start_pos = Some(mp);
        self.initial_view = im_view_initial;
    }

    fn unset_mouse_start_zoom(&mut self) {
        self.mouse_pressed_start_pos = None;
        self.initial_view = None;
    }

    fn mouse_pressed(
        &mut self,
        event: &WinitInputHelper,
        _shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        world: World,
        history: History,
    ) -> (World, History) {
        if event.mouse_pressed(RIGHT_BTN) {
            self.mover.move_mouse_pressed(mouse_pos);
        } else if let Some(mp) = mouse_pos {
            self.set_mouse_start_zoom(mp, Some(world.im_view().clone()));
        }
        (world, history)
    }

    fn mouse_released_left_btn(
        &mut self,
        mut world: World,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
    ) -> World {
        let shape_orig = world.shape_orig();
        let bx = if let (Some(mps), Some(mr)) = (self.mouse_pressed_start_pos, mouse_pos) {
            make_zoom_on_release(mps, mr, shape_orig, shape_win, world.zoom_box())
                .or(*world.zoom_box())
        } else {
            *world.zoom_box()
        };
        world.set_zoom_box(bx, shape_win);
        self.unset_mouse_start_zoom();
        world
    }

    fn mouse_released(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if event.mouse_released(LEFT_BTN) {
            world = self.mouse_released_left_btn(world, shape_win, mouse_pos);
            (world, history)
        } else if event.mouse_released(RIGHT_BTN) {
            self.unset_mouse_start_zoom();
            (world, history)
        } else {
            (world, history)
        }
    }

    fn mouse_held(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if event.mouse_held(RIGHT_BTN) {
            (self.mover, world) = move_zoom_box(self.mover, world, mouse_pos, shape_win);
            (world, history)
        } else if event.mouse_held(LEFT_BTN) {
            if let (Some(mps), Some(m)) = (self.mouse_pressed_start_pos, mouse_pos) {
                let initial_view = self.initial_view.clone();
                world.set_im_view(draw_zoombox_on_view(
                    initial_view.unwrap(),
                    BB::from_points(to_u32(mps), to_u32(m)),
                    &Rgb([255, 255, 255]),
                ));
            }
            (world, history)
        } else {
            (world, history)
        }
    }

    fn key_pressed(
        &mut self,
        _event: &WinitInputHelper,
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
            mouse_pressed_start_pos: None,
            initial_view: None,
            mover: Mover::new(),
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
            [
                (mouse_pressed, LEFT_BTN),
                (mouse_pressed, RIGHT_BTN),
                (mouse_released, LEFT_BTN),
                (mouse_held, LEFT_BTN),
                (mouse_held, RIGHT_BTN)
            ],
            [(key_pressed, VirtualKeyCode::Back)]
        )
    }
}

#[cfg(test)]
use {crate::result::RvResult, image::DynamicImage, std::collections::HashMap};
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
        let mpp = (mpp.0 as u32, mpp.1 as u32);
        let mph = (mph.0 as u32, mph.1 as u32);
        let shape_orig = Shape { w: 80, h: 80 };
        assert_eq!(follow_zoom_box(mpp, mph, shape_orig, zoom_box), expected);
    }
    test((4, 4), (12, 8), mk_z(12, 16, 40, 40), mk_z(4, 12, 40, 40));
    Ok(())
}
#[test]
fn test_on_mouse_pressed() -> RvResult<()> {
    let shape_win = Shape { w: 250, h: 500 };
    let mouse_pos = Some((30, 45));
    let im_orig = DynamicImage::ImageRgb8(ViewImage::new(250, 500));
    let mut z = Zoom::new();
    let world = World::from_real_im(im_orig, HashMap::new(), "".to_string(), shape_win);
    let history = History::new();
    let im_view_old = world.im_view().clone();
    let im_orig_old = world.data.clone();
    let event = WinitInputHelper::new();
    let (res, _) = z.mouse_pressed(&event, shape_win, mouse_pos, world, history);
    assert_eq!(*res.im_view(), im_view_old);
    assert_eq!(res.data, im_orig_old);
    assert_eq!(z.mouse_pressed_start_pos, mouse_pos);
    Ok(())
}

#[test]
fn test_on_mouse_released() -> RvResult<()> {
    let shape_win = Shape { w: 250, h: 500 };
    let mouse_pos = Some((30, 70));
    let im_orig = DynamicImage::ImageRgb8(ViewImage::new(250, 500));
    let mut z = Zoom::new();
    let world = World::from_real_im(im_orig, HashMap::new(), "".to_string(), shape_win);
    z.set_mouse_start_zoom((40, 80), Some(world.im_view().clone()));

    let world = z.mouse_released_left_btn(world, shape_win, mouse_pos);
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
    assert_eq!(z.mouse_pressed_start_pos, None);
    Ok(())
}

#[test]
fn test_on_mouse_held() {}
