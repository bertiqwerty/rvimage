use image::imageops;
use winit::event::VirtualKeyCode;

use crate::{
    history::{History, Record},
    make_tool_transform,
    util::{Event, Shape},
    world::{ImsRaw, World},
};

use super::Tool;

/// rotate 90 degrees counter clockwise
fn rot90(ims: &ImsRaw) -> ImsRaw {
    let mut ims = ims.clone();
    ims.apply(
        |im| im.rotate270(),
        |mask| mask.as_ref().map(imageops::rotate270),
    );
    ims
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
                ims_raw: world.ims_raw().clone(),
                file_label_idx: None,
                folder_label: None,
            });
            *world.ims_raw_mut() = rot90(world.ims_raw());
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
