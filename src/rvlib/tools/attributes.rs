use super::Manipulate;
use crate::{
    annotations_accessor, annotations_accessor_mut,
    events::Events,
    history::History,
    make_tool_transform,
    result::trace_ok,
    tools_data::{
        self,
        attributes_data::{self, set_attrmap_val, AttrMap},
    },
    tools_data_accessors,
    world::World,
};
use std::mem;
const MISSING_DATA_MSG: &str = "Missing data for Attributes";
pub const ACTOR_NAME: &str = "Attributes";
annotations_accessor_mut!(ACTOR_NAME, attributes_mut, "Attribute didn't work", AttrMap);
annotations_accessor!(ACTOR_NAME, attributes, "Attribute didn't work", AttrMap);
tools_data_accessors!(
    ACTOR_NAME,
    MISSING_DATA_MSG,
    attributes_data,
    AttributesToolData,
    attributes,
    attributes_mut
);
#[derive(Clone, Copy, Debug)]
pub struct Attributes;

impl Manipulate for Attributes {
    fn new() -> Self
    where
        Self: Sized,
    {
        Self
    }

    fn on_activate(&mut self, mut world: World) -> World {
        let data = get_data_mut(&mut world);
        if let Some(data) = trace_ok(data) {
            data.menu_active = true;
        }
        world
    }
    fn on_deactivate(&mut self, mut world: World) -> World {
        let data = get_data_mut(&mut world);
        if let Some(data) = trace_ok(data) {
            data.menu_active = true;
        }
        world
    }
    fn on_filechange(&mut self, mut world: World, history: History) -> (World, History) {
        let annos = get_annos_mut(&mut world).map(|annos| mem::take(annos));
        let data = get_specific_mut(&mut world);

        if let (Some(data), Some(mut annos)) = (data, annos) {
            for (attr_name, attr_type) in data.attr_names().iter().zip(data.attr_types().iter()) {
                if !annos.contains_key(attr_name) {
                    set_attrmap_val(&mut annos, attr_name, attr_type);
                }
            }
            let attr_buffers: Vec<String> = data
                .attr_names()
                .iter()
                .map(|attr_name| annos.get(attr_name).unwrap().to_string())
                .collect();
            for (i, buffer) in attr_buffers.into_iter().enumerate() {
                *data.attr_buffer_mut(i) = buffer;
            }
            if let Some(annos_) = get_annos_mut(&mut world) {
                *annos_ = annos;
            }
        }
        (world, history)
    }
    fn events_tf(&mut self, world: World, history: History, _event: &Events) -> (World, History) {
        make_tool_transform!(self, world, history, event, [])
    }
}
