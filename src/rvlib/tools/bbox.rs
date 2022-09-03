use crate::{
    anno_data_initializer,
    annotations::{Annotate, Annotations, BboxAnnotations},
    annotations_accessor, annotations_accessor_mut,
    history::{History, Record},
    make_tool_transform,
    util::{self, mouse_pos_to_orig_pos, orig_pos_to_view_pos, shape_unscaled, Shape, BB},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};
use crate::{
    tools::{core::Mover, Manipulate},
    util::to_i64,
};
use std::collections::HashMap;
use std::mem;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::core::InitialView;

const ACTOR_NAME: &str = "BBox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";
const CORNER_TOL_DENOMINATOR: u32 = 5000;

fn find_closest_boundary_idx(pos: (u32, u32), bbs: &[BB]) -> Option<usize> {
    bbs.iter()
        .enumerate()
        .filter(|(_, bb)| bb.contains(pos))
        .map(|(i, bb)| {
            let dx = (bb.x as i64 - pos.0 as i64).abs();
            let dw = ((bb.x + bb.w) as i64 - pos.0 as i64).abs();
            let dy = (bb.y as i64 - pos.1 as i64).abs();
            let dh = ((bb.y + bb.h) as i64 - pos.1 as i64).abs();
            (i, dx.min(dw).min(dy).min(dh))
        })
        .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap())
        .map(|(i, _)| i)
}

fn resize_bbs(
    mut bbs: Vec<BB>,
    selected_bbs: &[bool],
    x_shift: i32,
    y_shift: i32,
    shape_orig: Shape,
) -> Vec<BB> {
    let selected_idxs = selected_bbs
        .iter()
        .enumerate()
        .filter(|(_, x)| **x)
        .map(|(i, _)| i);
    for idx in selected_idxs {
        if let Some(bb) = bbs[idx].extend_max(x_shift, y_shift, shape_orig) {
            bbs[idx] = bb;
        }
    }
    bbs
}

fn move_bbs(
    mut bbs: Vec<BB>,
    selected_bbs: &[bool],
    x_shift: i32,
    y_shift: i32,
    shape_orig: Shape,
) -> Vec<BB> {
    let selected_idxs = selected_bbs
        .iter()
        .enumerate()
        .filter(|(_, x)| **x)
        .map(|(i, _)| i);
    for idx in selected_idxs {
        if let Some(bb) = bbs[idx].translate(x_shift, y_shift, shape_orig) {
            bbs[idx] = bb;
        }
    }
    bbs
}

/// returns index of the bounding box and the index of the closest close corner
fn find_close_corner(orig_pos: (u32, u32), bbs: &[BB], tolerance: i64) -> Option<(usize, usize)> {
    let opf = to_i64(orig_pos);
    bbs.iter()
        .enumerate()
        .map(|(bb_idx, bb)| {
            let (min_corner_idx, min_corner_dist) = bb
                .corners()
                .map(to_i64)
                .map(|c| (opf.0 - c.0).pow(2) + (opf.1 - c.1).pow(2))
                .enumerate()
                .min_by_key(|x| x.1)
                .unwrap();
            (bb_idx, min_corner_idx, min_corner_dist)
        })
        .filter(|(_, _, c_dist)| c_dist <= &tolerance)
        .min_by_key(|(_, _, c_dist)| *c_dist)
        .map(|(bb_idx, c_idx, _)| (bb_idx, c_idx))
}

anno_data_initializer!(ACTOR_NAME, Bbox, BboxAnnotations);
annotations_accessor_mut!(ACTOR_NAME, Bbox, BboxAnnotations, MISSING_ANNO_MSG);
annotations_accessor!(ACTOR_NAME, Bbox, BboxAnnotations, MISSING_ANNO_MSG);

#[derive(Clone, Debug)]
pub struct BBox {
    prev_pos: Option<(usize, usize)>,
    initial_view: InitialView,
    mover: Mover,
}

impl BBox {
    fn draw_on_view(&self, mut world: World, shape_win: Shape) -> World {
        let im_view = get_annos(&world).bbox().draw_on_view(
            self.initial_view.image().clone().unwrap(),
            world.zoom_box(),
            world.data.shape(),
            shape_win,
        );
        world.set_im_view(im_view);
        world
    }
    fn mouse_pressed(
        &mut self,
        _event: &WinitInputHelper,
        _shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        world: World,
        history: History,
    ) -> (World, History) {
        self.mover.move_mouse_pressed(mouse_pos);
        (world, history)
    }
    fn mouse_held(
        &mut self,
        _event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        let orig_shape = world.data.shape();
        let zoom_box = *world.zoom_box();
        let move_boxes = |mpso, mpo| {
            let annos = get_annos_mut(&mut world).bbox_mut();
            for (bb, is_bb_selected) in annos.bbs.iter_mut().zip(annos.selected_bbs.iter()) {
                if *is_bb_selected {
                    if let Some(bb_moved) = bb.follow_movement(mpso, mpo, orig_shape) {
                        *bb = bb_moved;
                    }
                }
            }
            Some(())
        };
        self.mover
            .move_mouse_held(move_boxes, mouse_pos, shape_win, orig_shape, &zoom_box);
        world = self.draw_on_view(world, shape_win);
        (world, history)
    }
    fn mouse_released(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let mp_orig =
            mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
        let pp_orig = mouse_pos_to_orig_pos(
            self.prev_pos,
            world.shape_orig(),
            shape_win,
            world.zoom_box(),
        );
        if let (Some(mp), Some(pp)) = (mp_orig, pp_orig) {
            // second click new bb
            let annos = get_annos_mut(&mut world).bbox_mut();
            annos.bbs.push(BB::from_points(mp, pp));
            annos.selected_bbs.push(false);
            history.push(Record::new(world.data.clone(), ACTOR_NAME));

            self.prev_pos = None;
        } else if event.key_held(VirtualKeyCode::LControl) {
            let annos = get_annos_mut(&mut world).bbox_mut();
            // selection
            let idx = mp_orig
                .and_then(|(x, y)| find_closest_boundary_idx((x as u32, y as u32), &annos.bbs));
            if let Some(i) = idx {
                annos.selected_bbs[i] = !annos.selected_bbs[i];
            }
            world = self.draw_on_view(world, shape_win);
        } else {
            let shape_orig = world.data.shape();
            let unscaled = shape_unscaled(world.zoom_box(), shape_orig);
            let tolerance = (unscaled.w * unscaled.h / CORNER_TOL_DENOMINATOR).max(2);
            let close_corner = mp_orig.and_then(|mp| {
                find_close_corner(mp, &get_annos(&world).bbox().bbs, tolerance as i64)
            });
            if let Some((bb_idx, idx)) = close_corner {
                // move an existing corner
                let annos = get_annos_mut(&mut world).bbox_mut();
                let bb = annos.bbs.remove(bb_idx);
                let oppo_corner = bb.opposite_corner(idx);
                annos.selected_bbs.remove(bb_idx);
                self.prev_pos =
                    orig_pos_to_view_pos(oppo_corner, shape_orig, shape_win, world.zoom_box())
                        .map(|(x, y)| (x as usize, y as usize));
            } else {
                // first click new bb
                self.prev_pos = mouse_pos;
            }
        }
        (world, history)
    }
    fn key_held(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        let shape_orig = world.data.shape();
        let annos = get_annos_mut(&mut world).bbox_mut();
        let taken_bbs = mem::take(&mut annos.bbs);
        if util::with_control(VirtualKeyCode::Up, |x| event.key_held(x)) {
            annos.bbs = resize_bbs(taken_bbs, &annos.selected_bbs, 0, -1, shape_orig);
        } else if util::with_control(VirtualKeyCode::Down, |x| event.key_held(x)) {
            annos.bbs = resize_bbs(taken_bbs, &annos.selected_bbs, 0, 1, shape_orig);
        } else if util::with_control(VirtualKeyCode::Right, |x| event.key_held(x)) {
            annos.bbs = resize_bbs(taken_bbs, &annos.selected_bbs, 1, 0, shape_orig);
        } else if util::with_control(VirtualKeyCode::Left, |x| event.key_held(x)) {
            annos.bbs = resize_bbs(taken_bbs, &annos.selected_bbs, -1, 0, shape_orig);
        } else if event.key_held(VirtualKeyCode::Up) {
            annos.bbs = move_bbs(taken_bbs, &annos.selected_bbs, 0, -1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Down) {
            annos.bbs = move_bbs(taken_bbs, &annos.selected_bbs, 0, 1, shape_orig);
        } else if event.key_held(VirtualKeyCode::Right) {
            annos.bbs = move_bbs(taken_bbs, &annos.selected_bbs, 1, 0, shape_orig);
        } else if event.key_held(VirtualKeyCode::Left) {
            annos.bbs = move_bbs(taken_bbs, &annos.selected_bbs, -1, 0, shape_orig);
        }
        world = self.draw_on_view(world, shape_win);
        world.update_view(shape_win);
        (world, history)
    }
    fn key_released(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let annos = get_annos_mut(&mut world).bbox_mut();
        if event.key_released(VirtualKeyCode::Back) {
            annos.bbs = vec![];
            annos.selected_bbs = vec![];
        } else {
            let bbs = mem::take(&mut annos.bbs);
            let selected_bbs = mem::take(&mut annos.selected_bbs);
            let keep_indices = selected_bbs
                .iter()
                .enumerate()
                .filter(|(_, is_selected)| !**is_selected)
                .map(|(i, _)| i);
            let bbs = keep_indices.clone().map(|i| bbs[i]).collect::<Vec<_>>();
            // the selected ones have been deleted hence all remaining ones are unselected
            let selected_bbs = vec![false; bbs.len()];

            annos.bbs = bbs;
            annos.selected_bbs = selected_bbs;
        }
        world = self.draw_on_view(world, shape_win);
        world.update_view(shape_win);
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        (world, history)
    }
}

impl Manipulate for BBox {
    fn new() -> Self {
        Self {

            prev_pos: None,
            initial_view: InitialView::new(),
            mover: Mover::new(),
        }
    }

    fn on_deactivate(
        &mut self,
        world: World,
        history: History,
        _shape_win: Shape,
    ) -> (World, History) {
        self.prev_pos = None;
        self.initial_view = InitialView::new();
        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
    ) -> (World, History) {
        world = initialize_anno_data(world);
        self.initial_view.update(&world, shape_win);
        let mp_orig =
            mouse_pos_to_orig_pos(mouse_pos, world.data.shape(), shape_win, world.zoom_box());
        let pp_orig = mouse_pos_to_orig_pos(
            self.prev_pos,
            world.data.shape(),
            shape_win,
            world.zoom_box(),
        );
        if let (Some(mp), Some(pp)) = (mp_orig, pp_orig) {
            // animation
            world = self.draw_on_view(world, shape_win);
            let tmp_annos = BboxAnnotations {
                bbs: vec![BB::from_points(mp, pp)],
                selected_bbs: vec![false],
            };
            let mut im_view = world.take_view();
            im_view =
                tmp_annos.draw_on_view(im_view, world.zoom_box(), world.data.shape(), shape_win);
            world.set_im_view(im_view);
        }
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [
                (mouse_pressed, RIGHT_BTN),
                (mouse_held, RIGHT_BTN),
                (mouse_released, LEFT_BTN)
            ],
            [
                (key_released, VirtualKeyCode::Back),
                (key_released, VirtualKeyCode::Delete),
                (key_held, VirtualKeyCode::Down),
                (key_held, VirtualKeyCode::Up),
                (key_held, VirtualKeyCode::Left),
                (key_held, VirtualKeyCode::Right)
            ]
        )
    }
}
#[cfg(test)]
fn make_test_bbs() -> Vec<BB> {
    vec![
        BB {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        },
        BB {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
        },
        BB {
            x: 9,
            y: 9,
            w: 10,
            h: 10,
        },
    ]
}
#[test]
fn test_find_idx() {
    let bbs = make_test_bbs();
    assert_eq!(find_closest_boundary_idx((0, 20), &bbs), None);
    assert_eq!(find_closest_boundary_idx((0, 0), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((3, 8), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((7, 14), &bbs), Some(1));
    assert_eq!(find_closest_boundary_idx((7, 15), &bbs), None);
    assert_eq!(find_closest_boundary_idx((8, 8), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((10, 12), &bbs), Some(2));
}
#[test]
fn test_bbs() {
    let bbs = make_test_bbs();
    let shape_orig = Shape { w: 100, h: 100 };
    let moved = move_bbs(bbs.clone(), &[false, true, true], 0, 1, shape_orig);
    assert_eq!(moved[0], bbs[0]);
    assert_eq!(BB::from_points((5, 6), (15, 16)), moved[1]);
    assert_eq!(BB::from_points((9, 10), (19, 20)), moved[2]);
    let resized = resize_bbs(bbs.clone(), &[false, true, true], -1, 1, shape_orig);
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(BB::from_points((5, 5), (14, 16)), resized[1]);
    assert_eq!(BB::from_points((9, 9), (18, 20)), resized[2]);
}
