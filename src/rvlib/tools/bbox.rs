use super::{core::Mover, Manipulate};
use crate::{
    history::{History, Record},
    make_tool_transform,
    tools::core,
    types::ViewImage,
    util::{self, mouse_pos_to_orig_pos, to_u32, Shape, BB},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};
use image::Rgb;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

const ACTOR_NAME: &str = "BBox";
const ALPHA: u8 = 90;
const ALPHA_SELECTED: u8 = 170;

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

fn draw_bbs<'a, I1: Iterator<Item = &'a BB>, I2: Iterator<Item = &'a bool>>(
    mut im: ViewImage,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
    bbs: I1,
    selected_bbs: I2,
    color: &Rgb<u8>,
) -> ViewImage {
    for (bb, is_selected) in bbs.zip(selected_bbs) {
        let alpha = if *is_selected { ALPHA_SELECTED } else { ALPHA };
        let f_inner_color = |rgb: &Rgb<u8>| util::apply_alpha(rgb, color, alpha);
        let view_corners = bb.to_view_corners(shape_orig, shape_win, zoom_box);
        im = core::draw_bx_on_image(im, view_corners.0, view_corners.1, color, f_inner_color);
    }
    im
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
    fn draw_bbs_on_view(&self, mut world: World, shape_win: Shape) -> World {

        let im_view = draw_bbs(
            self.initial_view.clone().unwrap(),
            world.ims_raw.shape(),
            shape_win,
            world.zoom_box(),
            self.bbs.iter(),
            self.selected_bbs.iter(),
            &Rgb([255, 255, 255]),
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
        world = self.draw_bbs_on_view(world, shape_win);
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
            world = self.draw_bbs_on_view(world, shape_win);
            history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
            self.prev_pos = None;
        } else {
            // first click
            if event.key_held(VirtualKeyCode::LControl) {
                let idx = mp_orig
                    .and_then(|(x, y)| find_closest_boundary_idx((x as u32, y as u32), &self.bbs));
                if let Some(i) = idx {
                    self.selected_bbs[i] = !self.selected_bbs[i];
                }
                world = self.draw_bbs_on_view(world, shape_win);
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
            world = self.draw_bbs_on_view(world, shape_win);
            world.update_view(shape_win);
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
            world.set_im_view(core::draw_bx_on_view(
                iv,
                to_u32(mp),
                to_u32(pp),
                &Rgb([255, 255, 255]),
            ));
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
    assert_eq!(find_closest_boundary_idx((0, 20), &bbs), None);
    assert_eq!(find_closest_boundary_idx((0, 0), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((3, 8), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((7, 14), &bbs), Some(1));
    assert_eq!(find_closest_boundary_idx((7, 15), &bbs), None);
    assert_eq!(find_closest_boundary_idx((8, 8), &bbs), Some(0));
    assert_eq!(find_closest_boundary_idx((10, 12), &bbs), Some(2));
    Ok(())
}
