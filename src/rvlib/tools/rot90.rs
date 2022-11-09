use crate::{
    domain::Shape,
    history::{History, Record},
    make_tool_transform,
    world::{DataRaw, World},
};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::Manipulate;

const ACTOR_NAME: &str = "Rot90";

/// rotate 90 degrees counter clockwise
fn rot90(ims: &DataRaw) -> DataRaw {
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
    ) -> (World, History) {
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        world = World::new(rot90(&world.data), *world.zoom_box(), shape_win);
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
            [(key_pressed, VirtualKeyCode::R)]
        )
    }
}
