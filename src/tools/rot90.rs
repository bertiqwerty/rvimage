use image::imageops;
use winit::event::VirtualKeyCode;

use crate::{
    make_tool_transform,
    util::{Event, Shape},
    world::World,
    ImageType,
};

use super::Tool;

/// rotate 90 degrees counter clockwise
fn rot90(im: &ImageType) -> ImageType {
    imageops::rotate270(im)
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
    ) -> World {
        if key == VirtualKeyCode::R {
            *world.im_orig_mut() = rot90(world.im_orig());
        }
        world
    }
}

impl Tool for Rot90 {
    fn new() -> Self {
        Self {}
    }

    fn events_tf<'a>(
        &'a mut self,
        world: World,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &Event,
    ) -> World {
        make_tool_transform!(
            self,
            world,
            shape_win,
            mouse_pos,
            event,
            [],
            [VirtualKeyCode::R]
        )
    }
}
