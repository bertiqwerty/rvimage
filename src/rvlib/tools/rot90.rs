use crate::{
    annotations_accessor, annotations_accessor_mut,
    events::{Events, KeyCode},
    history::{History, Record},
    make_tool_transform,
    tools_data::rot90_data::NRotations,
    world::{DataRaw, World},
};

use super::Manipulate;

pub const ACTOR_NAME: &str = "Rot90";
annotations_accessor_mut!(ACTOR_NAME, rot90_mut, "Rotation 90 didn't work", NRotations);
annotations_accessor!(ACTOR_NAME, rot90, "Rotation 90 didn't work", NRotations);

/// rotate 90 degrees counter clockwise
fn rot90(ims: &DataRaw, n_rotations: NRotations) -> DataRaw {
    let mut ims = ims.clone();
    match n_rotations {
        NRotations::Zero => (),
        NRotations::One => ims.apply(|im| im.rotate270()),
        NRotations::Two => ims.apply(|im| im.rotate180()),
        NRotations::Three => ims.apply(|im| im.rotate90()),
    }
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
        world = World::new(rot90(&world.data, NRotations::One), *world.zoom_box());
        if let Some(anno) = get_annos_mut(&mut world) {
            *anno = anno.increase();
        }
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        (world, history)
    }
}

impl Manipulate for Rot90 {
    fn new() -> Self {
        Self {}
    }

    fn on_filechange(&mut self, mut world: World, history: History) -> (World, History) {
        if let Some(nrot) = get_annos_if_some(&world) {
            world = World::new(rot90(&world.data, *nrot), *world.zoom_box());
        }
        (world, history)
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
