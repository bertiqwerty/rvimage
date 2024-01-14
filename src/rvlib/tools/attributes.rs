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
        let annos = get_annos_mut(&mut world).map(mem::take);
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
        let current = get_annos(&world).cloned();
        if let Some(data) = get_specific_mut(&mut world) {
            data.current_attr_map = current;
        }
        (world, history)
    }
    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        _event: &Events,
    ) -> (World, History) {
        let populate_new_attr = get_specific(&world).map(|d| d.options.populate_new_attr);
        if populate_new_attr == Some(true) {
            let attr_map_tmp = get_annos_mut(&mut world).map(mem::take);
            let data = get_specific_mut(&mut world);
            if let (Some(mut attr_map_tmp), Some(data)) = (attr_map_tmp, data) {
                set_attrmap_val(&mut attr_map_tmp, &data.new_attr, &data.new_attr_type);
                if let Some(a) = get_annos_mut(&mut world) {
                    *a = attr_map_tmp.clone();
                }
                if let Some(data) = get_specific_mut(&mut world) {
                    data.current_attr_map = Some(attr_map_tmp);
                }
            }
            if let Some(populate_new_attr) =
                get_specific_mut(&mut world).map(|d| &mut d.options.populate_new_attr)
            {
                *populate_new_attr = false;
            }
        }
        let update_current_attr_map =
            get_specific(&world).map(|d| d.options.update_current_attr_map);
        if update_current_attr_map == Some(true) {
            let current_from_menu_clone =
                get_specific(&world).and_then(|d| d.current_attr_map.clone());
            println!("current_from_menu_clone: {:?}", current_from_menu_clone);
            if let (Some(mut cfm), Some(anno)) =
                (current_from_menu_clone, get_annos_mut(&mut world))
            {
                *anno = mem::take(&mut cfm);
            }
            if let Some(update_current_attr_map) =
                get_specific_mut(&mut world).map(|d| &mut d.options.update_current_attr_map)
            {
                *update_current_attr_map = false;
            }
        }
        make_tool_transform!(self, world, history, event, [])
    }
}
