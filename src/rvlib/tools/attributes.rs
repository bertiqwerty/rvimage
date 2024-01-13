use crate::{
    annotations_accessor, annotations_accessor_mut,
    events::{Events, KeyCode},
    history::{History, Record},
    make_tool_transform,
    tools_data::attributes_data::AttrMap,
    world::{DataRaw, World},
};

use super::Manipulate;

pub const ACTOR_NAME: &str = "Attribute";
annotations_accessor_mut!(ACTOR_NAME, attributes_mut, "Attribute didn't work", AttrMap);
annotations_accessor!(ACTOR_NAME, attributes, "Attribute didn't work", AttrMap);

#[derive(Clone, Copy, Debug)]
pub struct Attributes;

impl Manipulate for Attributes {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn events_tf(&mut self, world: World, history: History, event: &Events) -> (World, History) {
        make_tool_transform!(self, world, history, event, [])
    }
}
