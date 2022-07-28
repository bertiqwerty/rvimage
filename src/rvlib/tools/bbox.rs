use super::Manipulate;
use crate::{
    history::{History, Record},
    make_tool_transform,
    tools::core,
    util::{mouse_pos_to_orig_pos, Shape},
    world::World,
    LEFT_BTN, RIGHT_BTN, types::ViewImage,
};
use image::{Rgb, Rgba};
use std::mem;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

const ACTOR_NAME: &str = "BBox";

#[derive(Clone, Debug)]
pub struct BBox {
    prev_pos: Option<(usize, usize)>,
    initial_view: Option<ViewImage>,
}

impl BBox {
    fn mouse_released(
        &mut self,
        btn: usize,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if btn == LEFT_BTN {
            let mp_orig =
                mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
            let pp_orig = 
                mouse_pos_to_orig_pos(self.prev_pos, world.shape_orig(), shape_win, world.zoom_box());
            if let (Some(mp), Some(pp)) = (mp_orig, pp_orig) {
                *world.ims_raw.im_annotations_mut() = core::draw_bx_on_anno(
                    mem::take(&mut world.ims_raw.im_annotations_mut()),
                    (mp.0 as usize, mp.1 as usize),
                    (pp.0 as usize, pp.1 as usize),
                    Rgba([255, 255, 255, 255]),
                );
                let zoom_box = *world.zoom_box();
                world
                    .ims_raw
                    .annotation_on_view(&mut world.im_view, shape_win, &zoom_box);
                history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
                self.prev_pos = None;
            } else {
                self.prev_pos = mouse_pos;
                self.initial_view = Some(world.im_view.clone());
            }
        }
        (world, history)
    }
    fn key_pressed(
        &mut self,
        _key: VirtualKeyCode,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if world.ims_raw.has_annotations() {
            world.ims_raw.clear_annotations();
            world.update_view(shape_win);
            history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
        }
        (world, history)
    }
}

impl Manipulate for BBox {
    fn new() -> Self {
        Self { prev_pos: None, initial_view: None }
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
            let iv = self.initial_view.clone();
            world.im_view = core::draw_bx_on_view(
                iv.unwrap().clone(),
                mp,
                pp,
                Rgb([255, 255, 255]),
            );
        }
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [mouse_released],
            [VirtualKeyCode::Back]
        )
    }
}
