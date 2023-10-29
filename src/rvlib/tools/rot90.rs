use crate::{
    domain::Shape,
    events::{Events, KeyCode},
    history::{History, Record},
    make_tool_transform,
    world::{DataRaw, World},
};

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
        _events: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        world = World::new(rot90(&world.data), *world.zoom_box());
        (world, history)
    }
}

impl Manipulate for Rot90 {
    fn new() -> Self {
        Self {}
    }

    fn events_tf(&mut self, world: World, history: History, event: &Events) -> (World, History) {
        make_tool_transform!(
            self,
            world,
            history,
            event,
            [(pressed, KeyCode::R, key_pressed)]
        )
    }
}
