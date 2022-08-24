use crate::{
    history::{History, Record},
    make_tool_transform,
    util::Shape,
    world::{ImsRaw, World},
};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::{core::MetaData, Manipulate};

const ACTOR_NAME: &str = "Rot90";

/// rotate 90 degrees counter clockwise
fn rot90(ims: &ImsRaw) -> ImsRaw {
    let mut ims = ims.clone();
    ims.apply(|im| im.rotate270());
    ims
}
#[derive(Clone, Copy, Debug)]
pub struct Rot90;

impl Rot90 {
    fn key_pressed(
        &mut self,
        _event: &WinitInputHelper,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
        _meta_data: &MetaData,
    ) -> (World, History) {
        history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
        world = World::new(rot90(&world.ims_raw), *world.zoom_box(), shape_win);
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
        meta_data: &MetaData,
    ) -> (World, History) {
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            meta_data,
            [],
            [(key_pressed, VirtualKeyCode::R)]
        )
    }
}
