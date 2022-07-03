use crate::{
    history::{History, Record},
    make_tool_transform,
    util::Shape,
    world::{ImsRaw, World},
};
use image::imageops;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::Manipulate;

/// rotate 90 degrees counter clockwise
fn rot90(ims: &ImsRaw) -> ImsRaw {
    let mut ims = ims.clone();
    ims.apply(|im| im.rotate270(), |a| imageops::rotate270(&a));
    ims
}
#[derive(Clone, Copy, Debug)]
pub struct Rot90;

impl Rot90 {
    fn key_pressed(
        &mut self,
        key: VirtualKeyCode,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if key == VirtualKeyCode::R {
            history.push(Record::new(world.ims_raw().clone()));
            world = World::new(rot90(world.ims_raw()), *world.zoom_box(), shape_win);
        }
        (world, history)
    }
}

impl Manipulate for Rot90 {
    fn new() -> Self {
        Self {}
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
            [],
            [VirtualKeyCode::R]
        )
    }
}
