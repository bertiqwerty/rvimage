use image::Rgba;
use winit_input_helper::WinitInputHelper;
use imageproc::drawing;
use crate::{
    history::{History, Record},
    make_tool_transform,
    util::{mouse_pos_to_orig_pos, Shape},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};

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
            if !world.ims_raw().has_annotations() {
                world.ims_raw_mut().create_annotations_layer();
            }
            let mp_orig =
                mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
            if let (Some(mp), Some(mp_prev)) = (mp_orig, self.prev_pos) {
                let start = (mp_prev.0 as f32, mp_prev.1 as f32);
                let end = (mp.0 as f32, mp.1 as f32);
                let clr = Rgba([255, 255, 255, 255]);
                drawing::draw_line_segment_mut(world.ims_raw_mut().im_annotations_mut().as_mut().unwrap(), start, end, clr);
                world.set_annotations_pixel(mp.0, mp.1, &[255, 255, 255, 255]);
                world.update_view(shape_win);
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
            history.push(Record::new(world.ims_raw().clone()));
            self.prev_pos = None;
        }
        (world, history)
    }
}

impl Manipulate for Brush {
    fn new() -> Self {
        Self { prev_pos: None }
    }

    fn events_tf<'a>(
        &'a mut self,
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
            []
        )
    }
}
