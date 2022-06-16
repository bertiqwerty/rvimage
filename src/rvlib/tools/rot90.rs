use image::DynamicImage;
use winit::event::VirtualKeyCode;

use crate::{
    history::{History, Record},
    make_tool_transform,
    util::{Event, Shape},
    world::World,
};

use super::Tool;

/// rotate 90 degrees counter clockwise
fn rot90(im: &DynamicImage) -> DynamicImage {
    im.rotate270()
}
#[derive(Clone, Copy, Debug)]
pub struct Rot90;

impl Rot90 {
    fn key_pressed(
        &mut self,
        key: VirtualKeyCode,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if key == VirtualKeyCode::R {
            history.push(Record {
                im_orig: world.im_orig().clone(),
                file_label_idx: None,
                folder_label: None,
            });
            *world.im_orig_mut() = rot90(world.im_orig());
        }
        (world, history)
    }
}

impl Tool for Rot90 {
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
            [],
            [VirtualKeyCode::R]
        )
    }
}
