use super::{core::Mover, Manipulate};
use crate::{
    history::{History, Record},
    make_tool_transform,
    tools::core,
    types::ViewImage,
    util::{mouse_pos_to_orig_pos, Shape, BB},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};
use image::Rgb;
use std::mem;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

const ACTOR_NAME: &str = "BBox";
const ALPHA: u8 = 90;
const ALPHA_SELECTED: u8 = 170;

fn find_bb_idx(pos: (u32, u32), bbs: &[BB]) -> Option<usize> {
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

fn draw_bbs(mut world: World, shape_win: Shape, bbs: &[BB], selected_bbs: &[bool]) -> World {
    world.ims_raw.clear_annotations();
    for (i, bb) in bbs.iter().enumerate() {
        let alpha = if selected_bbs[i] {
            ALPHA_SELECTED
        } else {
            ALPHA
        };
        *world.ims_raw.im_annotations_mut() = core::draw_bx_on_anno(
            mem::take(world.ims_raw.im_annotations_mut()),
            bb.min_usize(),
            bb.max_usize(),
            Rgb([255, 255, 255]),
            alpha,
        );
    }
    world.view_from_annotations(shape_win);
    world
}

#[derive(Clone, Debug)]
pub struct BBox {
    prev_pos: Option<(usize, usize)>,
    initial_view: Option<ViewImage>,
    bbs: Vec<BB>,
    selected_bbs: Vec<bool>,
    mover: Mover,
}

impl BBox {
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
        world: World,
        history: History,
    ) -> (World, History) {
        let move_boxes = |mpso, mpo| {
            for (bb, selected) in self.bbs.iter_mut().zip(self.selected_bbs.iter()) {
                if *selected {
                    if let Some(bb_moved) = bb.follow_movement(mpso, mpo, world.ims_raw.shape()) {
                        *bb = bb_moved;
                    }
                }
            }
            Some(())
        };
        self.mover.move_mouse_held(
            move_boxes,
            mouse_pos,
            shape_win,
            world.ims_raw.shape(),
            world.zoom_box(),
        );
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
            // second click
            self.bbs.push(BB::from_points(mp, pp));
            self.selected_bbs.push(false);
            world = draw_bbs(world, shape_win, &self.bbs, &self.selected_bbs);
            history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
            self.prev_pos = None;
        } else {
            // first click
            if event.key_held(VirtualKeyCode::LControl) {
                let idx = mp_orig.and_then(|(x, y)| find_bb_idx((x as u32, y as u32), &self.bbs));
                if let Some(i) = idx {
                    self.selected_bbs[i] = !self.selected_bbs[i];
                }
                world = draw_bbs(world, shape_win, &self.bbs, &self.selected_bbs);
            } else {
                self.prev_pos = mouse_pos;
                self.initial_view = mouse_pos.map(|_| world.im_view().clone());
            }
        }
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
        if event.key_released(VirtualKeyCode::Delete) {
            let keep_indices = self
                .selected_bbs
                .iter()
                .enumerate()
                .filter(|(_, is_selected)| !**is_selected)
                .map(|(i, _)| i);
            self.bbs = keep_indices.clone().map(|i| self.bbs[i]).collect();
            // the selected ones have been deleted hence all remaining ones are unselected
            self.selected_bbs.clear();
            self.selected_bbs.resize(self.bbs.len(), false);
            world = draw_bbs(world, shape_win, &self.bbs, &self.selected_bbs);
        } else if world.ims_raw.has_annotations() {
            world.ims_raw.clear_annotations();
            self.bbs = vec![];
            world.update_view(shape_win);
            history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
        }
        (world, history)
    }
}

impl Manipulate for BBox {
    fn new() -> Self {
        Self {
            prev_pos: None,
            initial_view: None,
            bbs: vec![],
            selected_bbs: vec![],
            mover: Mover::new(),
        }
    }

    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
    ) -> (World, History) {
        if let (Some(mp), Some(pp)) = (mouse_pos, self.prev_pos) {
            let iv = self.initial_view.clone().unwrap();
            world.set_im_view(core::draw_bx_on_view(iv, mp, pp, Rgb([255, 255, 255])));
        }
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [
                (mouse_released, LEFT_BTN),
                (mouse_pressed, RIGHT_BTN),
                (mouse_held, RIGHT_BTN)
            ],
            [
                (key_released, VirtualKeyCode::Back),
                (key_released, VirtualKeyCode::Delete)
            ]
        )
    }
}

#[cfg(test)]
use crate::result::RvResult;
#[test]
fn test_find_idx() -> RvResult<()> {
    let bbs = vec![
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
    ];
    assert_eq!(find_bb_idx((0, 20), &bbs), None);
    assert_eq!(find_bb_idx((0, 0), &bbs), Some(0));
    assert_eq!(find_bb_idx((3, 8), &bbs), Some(0));
    assert_eq!(find_bb_idx((7, 15), &bbs), Some(1));
    assert_eq!(find_bb_idx((8, 8), &bbs), Some(0));
    assert_eq!(find_bb_idx((10, 12), &bbs), Some(2));
    Ok(())
}
