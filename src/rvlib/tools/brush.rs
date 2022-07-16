use crate::{
    history::{History, Record},
    make_tool_transform,
    util::{mouse_pos_to_orig_pos, Shape},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};
use image::Rgba;
use imageproc::drawing;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::Manipulate;

#[derive(Clone, Copy, Debug)]
pub struct Brush {
    prev_pos: Option<(u32, u32)>,
}

impl Brush {
    fn mouse_held(
        &mut self,
        btn: usize,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if btn == LEFT_BTN {
            if !world.ims_raw.has_annotations() {
                world.ims_raw.create_annotations_layer();
            }
            let mp_orig =
                mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
            if let (Some(mp), Some(mp_prev)) = (mp_orig, self.prev_pos) {
                let start = (mp_prev.0 as f32, mp_prev.1 as f32);
                let end = (mp.0 as f32, mp.1 as f32);
                let clr = Rgba([255, 255, 255, 255]);
                drawing::draw_line_segment_mut(
                    world.ims_raw.im_annotations_mut().as_mut().unwrap(),
                    start,
                    end,
                    clr,
                );
                world.set_annotations_pixel(mp.0, mp.1, &[255, 255, 255, 255]);
                let zoom_box=world.zoom_box().clone();
                world.ims_raw.annotation_on_view(&mut world.im_view, shape_win, &zoom_box);
            }
            self.prev_pos = mp_orig;
        }
        (world, history)
    }

    fn mouse_released(
        &mut self,
        btn: usize,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        world: World,
        mut history: History,
    ) -> (World, History) {
        if btn == LEFT_BTN {
            history.push(Record::new(world.ims_raw.clone()));
            self.prev_pos = None;
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
        if world.ims_raw.im_annotations_mut().is_some() {
            world.ims_raw.clear_annotations();
            world.update_view(shape_win);
            history.push(Record::new(world.ims_raw.clone()));
        }
        (world, history)
    }
}

impl Manipulate for Brush {
    fn new() -> Self {
        Self { prev_pos: None }
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
            [mouse_held, mouse_released],
            [VirtualKeyCode::Back]
        )
    }
}
