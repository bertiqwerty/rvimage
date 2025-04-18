use tracing::info;

use super::Manipulate;
use crate::{
    annotations_accessor_mut,
    events::Events,
    file_util::PathPair,
    history::History,
    make_tool_transform,
    result::trace_ok_err,
    tools_data::{
        self,
        attributes_data::{self, set_attrmap_val, AttrMap, AttrVal},
    },
    tools_data_accessors,
    world::World,
    world_annotations_accessor,
};
use std::mem;
const MISSING_DATA_MSG: &str = "Missing data for Attributes";
pub const ACTOR_NAME: &str = "Attributes";
annotations_accessor_mut!(ACTOR_NAME, attributes_mut, "Attribute didn't work", AttrMap);
world_annotations_accessor!(ACTOR_NAME, attributes, "Attribute didn't work", AttrMap);
tools_data_accessors!(
    ACTOR_NAME,
    MISSING_DATA_MSG,
    attributes_data,
    AttributesToolData,
    attributes,
    attributes_mut
);

fn propagate_annos(
    mut annos: AttrMap,
    attr_names: &[String],
    to_propagate: &[(usize, AttrVal)],
) -> AttrMap {
    for (attr_idx, val) in to_propagate {
        if let Some(attr_val) = annos.get_mut(&attr_names[*attr_idx]) {
            *attr_val = val.clone();
        }
    }
    annos
}

fn propagate_buffer(
    mut attribute_buffer: Vec<String>,
    to_propagate: &[(usize, AttrVal)],
) -> Vec<String> {
    for (attr_idx, val) in to_propagate {
        attribute_buffer[*attr_idx] = val.to_string();
    }
    attribute_buffer
}
fn file_change(mut world: World) -> World {
    let annos = get_annos_mut(&mut world).map(mem::take);
    let data = get_specific_mut(&mut world);

    if let (Some(data), Some(mut annos)) = (data, annos) {
        for (attr_name, attr_val) in data.attr_names().iter().zip(data.attr_vals().iter()) {
            if !annos.contains_key(attr_name) {
                set_attrmap_val(&mut annos, attr_name, attr_val);
            }
        }

        // put string representations of the attribute values into the buffer
        let attr_buffers: Vec<String> = data
            .attr_names()
            .iter()
            .map(|attr_name| annos.get(attr_name).unwrap().to_string())
            .collect();
        let attr_buffers = propagate_buffer(attr_buffers, &data.to_propagate_attr_val);
        for (i, buffer) in attr_buffers.into_iter().enumerate() {
            *data.attr_value_buffer_mut(i) = buffer;
        }

        annos = propagate_annos(annos, data.attr_names(), &data.to_propagate_attr_val);

        if let Some(annos_) = get_annos_mut(&mut world) {
            *annos_ = annos;
        }
    }
    let current = get_annos(&world).cloned();
    if let Some(data) = get_specific_mut(&mut world) {
        data.current_attr_map = current;
    }
    world
}
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
        if let Some(data) = trace_ok_err(data) {
            data.menu_active = true;
        }
        file_change(world)
    }
    fn on_deactivate(&mut self, mut world: World) -> World {
        let data = get_data_mut(&mut world);
        if let Some(data) = trace_ok_err(data) {
            data.menu_active = false;
        }
        world
    }
    fn on_filechange(&mut self, world: World, history: History) -> (World, History) {
        (file_change(world), history)
    }
    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        _event: &Events,
    ) -> (World, History) {
        let is_addition_triggered = get_specific(&world).map(|d| d.options.is_addition_triggered);
        if is_addition_triggered == Some(true) {
            let attr_map_tmp = get_annos_mut(&mut world).map(mem::take);
            let data = get_specific_mut(&mut world);

            if let (Some(mut attr_map_tmp), Some(data)) = (attr_map_tmp, data) {
                let new_attr_name = data.new_attr_name.clone();
                if data.attr_names().contains(&new_attr_name) {
                    tracing::error!(
                        "New attribute {new_attr_name} could not be created, already exists"
                    );
                } else {
                    let new_attr_val = data.new_attr_val.clone();
                    for (_, (val_map, _)) in data.anno_iter_mut() {
                        set_attrmap_val(val_map, &new_attr_name, &new_attr_val);
                    }
                    set_attrmap_val(&mut attr_map_tmp, &new_attr_name, &new_attr_val);
                    if let Some(a) = get_annos_mut(&mut world) {
                        a.clone_from(&attr_map_tmp);
                    }
                    if let Some(data) = get_specific_mut(&mut world) {
                        data.current_attr_map = Some(attr_map_tmp);
                        data.push(new_attr_name, new_attr_val);
                    }
                }
            }
            if let Some(is_add_triggered_out) =
                get_specific_mut(&mut world).map(|d| &mut d.options.is_addition_triggered)
            {
                *is_add_triggered_out = false;
            }
        }
        let attr_data = get_specific_mut(&mut world);
        if let Some(attr_data) = attr_data {
            if let Some(rename_src_idx) = attr_data.options.rename_src_idx {
                let from_name = &attr_data.attr_names()[rename_src_idx].clone();
                let to_name = &attr_data.new_attr_name.clone();
                tracing::info!("Rename attribute {from_name} to {to_name}");
                attr_data.rename(from_name, to_name);
                attr_data.options.rename_src_idx = None;
            }
        }
        let is_update_triggered = get_specific(&world).map(|d| d.options.is_update_triggered);
        if is_update_triggered == Some(true) {
            info!("update attr");
            let current_from_menu_clone =
                get_specific(&world).and_then(|d| d.current_attr_map.clone());
            if let (Some(mut cfm), Some(anno)) =
                (current_from_menu_clone, get_annos_mut(&mut world))
            {
                *anno = mem::take(&mut cfm);
            }
            if let Some(update_current_attr_map) =
                get_specific_mut(&mut world).map(|d| &mut d.options.is_update_triggered)
            {
                *update_current_attr_map = false;
            }
        }
        if let Some(removal_idx) = get_specific(&world).map(|d| d.options.removal_idx) {
            let data = get_specific_mut(&mut world);
            if let (Some(data), Some(removal_idx)) = (data, removal_idx) {
                data.remove_attr(removal_idx);
            }
            if let Some(removal_idx) =
                get_specific_mut(&mut world).map(|d| &mut d.options.removal_idx)
            {
                *removal_idx = None;
            }
        }
        let is_export_triggered = get_specific(&world).map(|d| d.options.is_export_triggered);
        if is_export_triggered == Some(true) {
            let ssh_cfg = world.data.meta_data.ssh_cfg.clone();
            let attr_data = get_specific(&world);
            let export_only_opened_folder =
                attr_data.map(|d| d.options.export_only_opened_folder) == Some(true);
            let key_filter = if export_only_opened_folder {
                world
                    .data
                    .meta_data
                    .opened_folder
                    .as_ref()
                    .map(PathPair::path_relative)
            } else {
                None
            };
            let annos_str = get_specific(&world)
                .and_then(|d| trace_ok_err(d.serialize_annotations(key_filter)));
            if let (Some(annos_str), Some(data)) = (annos_str, get_specific(&world)) {
                if trace_ok_err(data.export_path.conn.write(
                    &annos_str,
                    &data.export_path.path,
                    ssh_cfg.as_ref(),
                ))
                .is_some()
                {
                    info!("exported annotations to {:?}", data.export_path.path);
                }
            }
            if let Some(is_export_triggered) =
                get_specific_mut(&mut world).map(|d| &mut d.options.is_export_triggered)
            {
                *is_export_triggered = false;
            }
        }
        make_tool_transform!(self, world, history, event, [])
    }
}
