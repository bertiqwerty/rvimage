use std::mem;

use crate::tools::{
    core::{draw_bx_on_view, MetaData, Mover},
    Manipulate,
};
use crate::{
    annotations::{Annotate, Annotations, BboxAnnotations},
    anno_data_initializer, annotations_accessor, annotations_accessor_mut,
    history::{History, Record},
    make_tool_transform,
    types::ViewImage,
    util::{mouse_pos_to_orig_pos, to_u32, Shape, BB},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};
use image::Rgb;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

const ACTOR_NAME: &str = "BBox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";

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

anno_data_initializer!(ACTOR_NAME, Bbox, BboxAnnotations);
annotations_accessor_mut!(ACTOR_NAME, Bbox, BboxAnnotations);
annotations_accessor!(ACTOR_NAME, Bbox, BboxAnnotations);

fn current_file_path(world: &World) -> &String {
    &get_annos(world)
        .expect(MISSING_ANNO_MSG)
        .bbox()
        .current_file_path
}
fn set_current_file_path(world: &mut World, cfp: String) {
    get_annos_mut(world).bbox_mut().set_current_file_path(cfp);
}

fn bbs_mut(world: &mut World) -> &mut (Vec<BB>, Vec<bool>) {
    get_annos_mut(world).bbox_mut().get_current_annos_mut()
}
#[derive(Clone, Debug)]
pub struct BBox {
    prev_pos: Option<(usize, usize)>,
    initial_view: Option<ViewImage>,
    mover: Mover,
}

impl BBox {
    fn draw_on_view(&self, mut world: World, shape_win: Shape) -> World {
        let im_view = get_annos(&world)
            .expect(MISSING_ANNO_MSG)
            .bbox()
            .draw_on_view(
                self.initial_view.clone().unwrap(),
                world.zoom_box(),
                world.ims_raw.shape(),
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
        let orig_shape = world.ims_raw.shape();
        let zoom_box = *world.zoom_box();
        let move_boxes = |mpso, mpo| {
            let (bbs, selecteds) = bbs_mut(&mut world);
            for (bb, selected) in bbs.iter_mut().zip(selecteds.iter()) {
                if *selected {
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
            // second click
            bbs_mut(&mut world).0.push(BB::from_points(mp, pp));
            bbs_mut(&mut world).1.push(false);
            history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));

            self.prev_pos = None;
        } else {
            // first click
            if event.key_held(VirtualKeyCode::LControl) {
                let (bbs, selected_bbs) = bbs_mut(&mut world);
                let idx =
                    mp_orig.and_then(|(x, y)| find_closest_boundary_idx((x as u32, y as u32), bbs));
                if let Some(i) = idx {
                    selected_bbs[i] = !selected_bbs[i];
                }
                world = self.draw_on_view(world, shape_win);
            } else {
                self.prev_pos = mouse_pos;
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
            let (bbs, selected_bbs) = mem::take(bbs_mut(&mut world));
            let keep_indices = selected_bbs
                .iter()
                .enumerate()
                .filter(|(_, is_selected)| !**is_selected)
                .map(|(i, _)| i);
            let bbs = keep_indices.clone().map(|i| bbs[i]).collect::<Vec<_>>();
            // the selected ones have been deleted hence all remaining ones are unselected
            let selected_bbs = vec![false; bbs.len()];

            *bbs_mut(&mut world) = (bbs, selected_bbs);

            world = self.draw_on_view(world, shape_win);
            world.update_view(shape_win);
        } else if world.ims_raw.has_annotations() {
            *bbs_mut(&mut world) = (vec![], vec![]);
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
            mover: Mover::new(),
        }
    }

    fn on_deactivate(
        &mut self,
        world: World,
        history: History,
        _shape_win: Shape,
        _meta_data: &MetaData,
    ) -> (World, History) {
        self.prev_pos = None;
        self.initial_view = None;
        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
        meta_data: &MetaData,
    ) -> (World, History) {
        
        world = initialize_anno_data(world);

        if let Some(mdfp) = meta_data.file_path {
            if current_file_path(&world) != mdfp {
                set_current_file_path(&mut world, mdfp.to_string());
                self.initial_view = Some(world.im_view().clone());
            }
        }
        if self.initial_view.is_none() {
            self.initial_view = Some(world.im_view().clone());
        }
        if let Some(iv) = &self.initial_view {
            if Shape::from_im(iv) != Shape::from_im(world.im_view()) {
                self.initial_view = Some(world.im_view().clone());
            }
        }
        if let (Some(mp), Some(pp)) = (mouse_pos, self.prev_pos) {
            world = self.draw_on_view(world, shape_win);
            let im_view = world.take_view();
            world.set_im_view(draw_bx_on_view(
                im_view,
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
