use image::{imageops, DynamicImage, Rgba};
use winit::event::VirtualKeyCode;

use crate::{
    history::{History, Record},
    make_tool_transform,
    util::{Event, Shape},
    world::World,
    LEFT_BTN, RIGHT_BTN,
};

use super::Manipulate;

#[derive(Clone, Copy, Debug)]
pub struct Brush;

impl Brush {
    fn mouse_held(
        &mut self,
        btn: usize,
        _shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if !world.ims_raw().has_annotations() {
            world.ims_raw_mut().create_annotations_layer();
        }
        if let Some(mp) = mouse_pos {
            world.ims_raw_mut().set_annotations_pixel(
                mp.0 as u32,
                mp.1 as u32,
                [255, 255, 255, 255],
            );
        }
        (world, history)
    }
    fn mouse_released(
        &mut self,
        btn: usize,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        (world, history)
    }
}

impl Manipulate for Brush {
    fn new() -> Self {
        Self {}
    }

    fn events_tf<'a>(
        &'a mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &Event,
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
