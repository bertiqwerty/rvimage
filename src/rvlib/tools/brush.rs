use std::{cmp::Ordering, mem};

use tracing::info;

use crate::{
    annotations_accessor_mut,
    domain::{BrushLine, PtF},
    events::{Events, KeyCode},
    history::{History, Record},
    make_tool_transform,
    result::{trace_ok, RvResult},
    tools::core::{check_recolorboxes, check_trigger_redraw},
    tools_data::{self, annotations::InstanceAnnotations, brush_data, ToolsData},
    tools_data::{annotations::BrushAnnotations, brush_mut},
    world::World,
    Line,
};

use super::{
    core::{label_change_key, map_released_key, on_selection_keys, ReleasedKey},
    Manipulate, BRUSH_NAME,
};

pub const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";
const MISSING_DATA_MSG: &str = "brush data not available";

annotations_accessor_mut!(ACTOR_NAME, brush_mut, MISSING_ANNO_MSG, BrushAnnotations);

const MAX_SELECT_DIST: f32 = 20.0;

fn get_data(world: &World) -> RvResult<&ToolsData> {
    tools_data::get(world, ACTOR_NAME, MISSING_DATA_MSG)
}

fn get_specific(world: &World) -> Option<&brush_data::BrushToolData> {
    tools_data::get_specific(tools_data::brush, get_data(world))
}
fn get_options(world: &World) -> Option<brush_data::Options> {
    get_specific(world).map(|d| d.options)
}

fn get_data_mut(world: &mut World) -> RvResult<&mut ToolsData> {
    tools_data::get_mut(world, ACTOR_NAME, MISSING_DATA_MSG)
}
fn get_specific_mut(world: &mut World) -> Option<&mut brush_data::BrushToolData> {
    tools_data::get_specific_mut(tools_data::brush_mut, get_data_mut(world))
}
fn get_options_mut(world: &mut World) -> Option<&mut brush_data::Options> {
    get_specific_mut(world).map(|d| &mut d.options)
}
fn are_brushlines_visible(world: &World) -> bool {
    get_options(world).map(|o| o.core_options.visible) == Some(true)
}

fn find_closest_brushline(annos: &InstanceAnnotations<BrushLine>, p: PtF) -> Option<(usize, f32)> {
    annos
        .elts()
        .iter()
        .enumerate()
        .map(|(i, line)| (i, line.line.dist_to_point(p)))
        .filter(|(_, dist)| dist.is_some())
        .map(|(i, dist)| (i, dist.unwrap()))
        .min_by(|(_, x), (_, y)| match x.partial_cmp(y) {
            Some(o) => o,
            None => Ordering::Greater,
        })
}

#[derive(Clone, Debug)]
pub struct Brush {}

impl Brush {
    fn mouse_pressed(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if !events.held_ctrl() {
            let options = get_options(&world);
            let cat_idx = get_specific(&world).map(|d| d.label_info.cat_idx_current);
            if let (Some(mp), Some(annos), Some(options), Some(cat_idx)) = (
                events.mouse_pos,
                get_annos_mut(&mut world),
                options,
                cat_idx,
            ) {
                let erase = options.erase;
                if erase {
                    let to_be_removed_line_idx = find_closest_brushline(annos, mp);
                    if let Some((idx, dist)) = to_be_removed_line_idx {
                        if dist < MAX_SELECT_DIST {
                            annos.remove(idx);
                            world.request_redraw_annotations(BRUSH_NAME, true)
                        }
                    }
                } else {
                    annos.add_elt(
                        BrushLine {
                            line: Line::new(),
                            intensity: options.intensity,
                            thickness: options.thickness,
                        },
                        cat_idx,
                    );
                }
            }
        }
        (world, history)
    }
    fn mouse_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if !events.held_ctrl() {
            let erase = get_options(&world).map(|o| o.erase);
            if let (Some(mp), Some(annos)) = (events.mouse_pos, get_annos_mut(&mut world)) {
                if erase != Some(true) {
                    if let Some(line) = annos.last_line_mut() {
                        let last_point = line.last_point();
                        let dist = if let Some(last_point) = last_point {
                            last_point.dist_square(&mp.into())
                        } else {
                            100
                        };
                        if dist >= 1 {
                            line.push(mp.into());
                        }
                    }
                    world.request_redraw_annotations(BRUSH_NAME, true)
                }
            }
        }
        (world, history)
    }

    fn mouse_released(
        &mut self,
        events: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if events.held_ctrl() {
            if let (Some(mp), Some(annos)) = (events.mouse_pos, get_annos_mut(&mut world)) {
                let to_be_selected_line_idx = find_closest_brushline(annos, mp);
                if let Some((idx, dist)) = to_be_selected_line_idx {
                    if dist < MAX_SELECT_DIST {
                        if annos.selected_mask()[idx] {
                            annos.deselect(idx);
                        } else {
                            annos.select(idx);
                        }
                        world.request_redraw_annotations(BRUSH_NAME, true)
                    }
                }
            }
        } else {
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
        }
        (world, history)
    }
    fn key_released(
        &mut self,
        events: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let released_key = map_released_key(events);
        if let Some(label_info) = get_specific_mut(&mut world).map(|s| &mut s.label_info) {
            *label_info = label_change_key(released_key, mem::take(label_info));
        }
        (world, history) = on_selection_keys(
            world,
            history,
            released_key,
            events.held_ctrl(),
            BRUSH_NAME,
            get_annos_mut,
            |world| get_specific_mut(world).map(|d| &mut d.clipboard),
        );
        match released_key {
            ReleasedKey::H if events.held_ctrl() => {
                // Hide all boxes (selected or not)
                if let Some(options_mut) = get_options_mut(&mut world) {
                    options_mut.core_options.visible = !options_mut.core_options.visible;
                }
                world.request_redraw_annotations(BRUSH_NAME, are_brushlines_visible(&world));
            }
            ReleasedKey::E => {
                // Hide all boxes (selected or not)
                if let Some(options_mut) = get_options_mut(&mut world) {
                    if options_mut.erase {
                        info!("stop erase via shortcut");
                    } else {
                        info!("start erase via shortcut");
                    }
                    options_mut.erase = !options_mut.erase;
                }
                world.request_redraw_annotations(BRUSH_NAME, are_brushlines_visible(&world));
            }
            _ => (),
        }
        (world, history)
    }
}

impl Manipulate for Brush {
    fn new() -> Self {
        Self {}
    }

    fn on_filechange(&mut self, mut world: World, history: History) -> (World, History) {
        let brush_data = get_specific_mut(&mut world);
        if let Some(brush_data) = brush_data {
            for (_, (anno, _)) in brush_data.anno_iter_mut() {
                anno.deselect_all();
            }
        }
        let options = get_options(&world);
        if let Some(options) = options {
            world.request_redraw_annotations(BRUSH_NAME, options.core_options.visible);
        }
        (world, history)
    }
    fn on_activate(&mut self, mut world: World, history: History) -> (World, History) {
        if let Some(data) = trace_ok(get_data_mut(&mut world)) {
            data.menu_active = true;
            let are_annos_visible = true;
            world.request_redraw_annotations(BRUSH_NAME, are_annos_visible);
        }
        (world, history)
    }
    fn on_deactivate(&mut self, mut world: World, history: History) -> (World, History) {
        if let Some(td) = world.data.tools_data_map.get_mut(BRUSH_NAME) {
            td.menu_active = false;
        }
        let are_boxes_visible = false;
        world.request_redraw_annotations(BRUSH_NAME, are_boxes_visible);
        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        events: &Events,
    ) -> (World, History) {
        world = check_trigger_redraw(world, BRUSH_NAME, |d| {
            brush_mut(d).map(|d| &mut d.options.core_options)
        });
        world = check_recolorboxes(
            world,
            BRUSH_NAME,
            |world| get_options_mut(world).map(|o| &mut o.core_options),
            |world| get_specific_mut(world).map(|d| &mut d.label_info),
        );
        make_tool_transform!(
            self,
            world,
            history,
            events,
            [
                (pressed, KeyCode::MouseLeft, mouse_pressed),
                (held, KeyCode::MouseLeft, mouse_held),
                (released, KeyCode::MouseLeft, mouse_released),
                (released, KeyCode::Back, key_released),
                (released, KeyCode::Delete, key_released),
                (released, KeyCode::A, key_released),
                (released, KeyCode::C, key_released),
                (released, KeyCode::D, key_released),
                (released, KeyCode::E, key_released),
                (released, KeyCode::H, key_released),
                (released, KeyCode::V, key_released),
                (released, KeyCode::Key1, key_released),
                (released, KeyCode::Key2, key_released),
                (released, KeyCode::Key3, key_released),
                (released, KeyCode::Key4, key_released),
                (released, KeyCode::Key5, key_released),
                (released, KeyCode::Key6, key_released),
                (released, KeyCode::Key7, key_released),
                (released, KeyCode::Key8, key_released),
                (released, KeyCode::Key9, key_released)
            ]
        )
    }
}
