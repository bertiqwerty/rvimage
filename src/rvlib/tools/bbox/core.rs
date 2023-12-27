use crate::{
    annotations_accessor, annotations_accessor_mut,
    domain::{Shape, BB},
    drawme::{Annotation, BboxAnnotation, Stroke},
    events::{Events, KeyCode},
    file_util,
    history::{History, Record},
    make_tool_transform,
    result::{trace_ok, RvResult},
    tools::{
        core::{check_trigger_redraw, Mover},
        Manipulate, BBOX_NAME,
    },
    tools_data::{self, annotations::BboxAnnotations, bbox_data, bbox_mut, ToolsData},
    world::World,
    GeoFig, Polygon,
};
use std::{iter, mem};

use super::on_events::{
    export_if_triggered, import_coco_if_triggered, map_released_key, on_key_released,
    on_mouse_held_right, on_mouse_released_left, on_mouse_released_right, KeyReleasedParams,
    MouseHeldParams, MouseReleaseParams, PrevPos,
};
pub const ACTOR_NAME: &str = "Bbox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";
const MISSING_DATA_MSG: &str = "bbox tools data not available";
annotations_accessor_mut!(ACTOR_NAME, bbox_mut, MISSING_ANNO_MSG, BboxAnnotations);
annotations_accessor!(ACTOR_NAME, bbox, MISSING_ANNO_MSG, BboxAnnotations);

pub(super) fn get_data(world: &World) -> RvResult<&ToolsData> {
    tools_data::get(world, ACTOR_NAME, MISSING_DATA_MSG)
}

pub(super) fn get_specific(world: &World) -> Option<&bbox_data::BboxSpecificData> {
    tools_data::get_specific(tools_data::bbox, get_data(world))
}
pub(super) fn get_options(world: &World) -> Option<bbox_data::Options> {
    get_specific(world).map(|d| d.options)
}

pub(super) fn get_data_mut(world: &mut World) -> RvResult<&mut ToolsData> {
    tools_data::get_mut(world, ACTOR_NAME, MISSING_DATA_MSG)
}
pub(super) fn get_specific_mut(world: &mut World) -> Option<&mut bbox_data::BboxSpecificData> {
    tools_data::get_specific_mut(tools_data::bbox_mut, get_data_mut(world))
}
pub(super) fn get_options_mut(world: &mut World) -> Option<&mut bbox_data::Options> {
    get_specific_mut(world).map(|d| &mut d.options)
}
pub(super) fn are_boxes_visible(world: &World) -> bool {
    get_options(world).map(|o| o.core_options.visible) != Some(false)
}

pub(super) fn paste(mut world: World, mut history: History) -> (World, History) {
    let clipboard = get_specific(&world).and_then(|d| d.clipboard.clone());
    if let Some(clipboard) = &clipboard {
        let cb_bbs = clipboard.geos();
        if !cb_bbs.is_empty() {
            let shape_orig = Shape::from_im(world.data.im_background());
            if let Some(a) = get_annos_mut(&mut world) {
                a.extend(
                    cb_bbs.iter().cloned(),
                    clipboard.cat_idxs().iter().copied(),
                    shape_orig,
                )
            }
        }
    }
    if let (Some(_), Some(specific_mut)) = (clipboard, get_specific_mut(&mut world)) {
        let are_boxes_visible = true;
        specific_mut.options.core_options.visible = are_boxes_visible;
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
    }

    (world, history)
}

pub(super) fn current_cat_idx(world: &World) -> Option<usize> {
    get_specific(world).map(|d| d.label_info.cat_idx_current)
}

fn check_recolorboxes(mut world: World) -> World {
    // check if re-color was triggered
    let options = get_options(&world);
    if options.map(|o| o.core_options.is_colorchange_triggered) == Some(true) {
        let data = get_specific_mut(&mut world);
        if let Some(data) = data {
            // we show annotations after recoloring
            let are_boxes_visible = true;
            data.label_info.new_random_colors();
            data.options.core_options.is_colorchange_triggered = false;
            data.options.core_options.visible = true;
            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        }
    }
    world
}

fn check_annoremove(mut world: World) -> World {
    let is_anno_rm_triggered = get_options(&world).map(|o| o.is_anno_rm_triggered);
    if is_anno_rm_triggered == Some(true) {
        let opened_folder = world
            .data
            .meta_data
            .opened_folder
            .as_ref()
            .map(|of| file_util::url_encode(of));

        // we show annotations after recoloring
        let are_boxes_visible = true;
        let data = get_specific_mut(&mut world);
        if let (Some(data), Some(opened_folder)) = (data, &opened_folder) {
            data.retain_fileannos_in_folder(opened_folder);
            data.options.is_anno_rm_triggered = false;
            data.options.core_options.visible = are_boxes_visible;
        }
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
    }
    world
}

fn check_cocoexport(mut world: World) -> World {
    // export label file if demanded
    let bbox_data = get_specific(&world);
    if let Some(bbox_data) = bbox_data {
        export_if_triggered(&world.data.meta_data, bbox_data);
        if let Some(o) = get_options_mut(&mut world) {
            o.core_options.is_export_triggered = false;
        }
    }
    world
}

fn check_cocoimport(mut world: World) -> World {
    // import coco if demanded
    let options = get_options(&world);
    if let Some(options) = options {
        if let Some(imported_data) = import_coco_if_triggered(
            &world.data.meta_data,
            if options.is_coco_import_triggered {
                get_specific(&world).map(|o| &o.coco_file)
            } else {
                None
            },
        ) {
            let are_boxes_visible = imported_data.options.core_options.visible;
            if let Some(data_mut) = get_specific_mut(&mut world) {
                *data_mut = imported_data;
                data_mut.options.is_coco_import_triggered = false;
            }
            world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        }
    }
    world
}

fn check_autopaste(mut world: World, mut history: History, auto_paste: bool) -> (World, History) {
    if world.data.meta_data.is_loading_screen_active == Some(false) && auto_paste {
        (world, history) = paste(world, history);
    }
    (world, history)
}

#[derive(Clone, Debug)]
pub struct Bbox {
    prev_pos: PrevPos,
    mover: Mover,
}

impl Bbox {
    fn mouse_pressed(
        &mut self,
        event: &Events,
        world: World,
        history: History,
    ) -> (World, History) {
        self.mover.move_mouse_pressed(event.mouse_pos);
        (world, history)
    }

    fn mouse_held(&mut self, event: &Events, world: World, history: History) -> (World, History) {
        let params = MouseHeldParams {
            mover: &mut self.mover,
        };
        on_mouse_held_right(event.mouse_pos, params, world, history)
    }

    fn mouse_released(
        &mut self,
        event: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let are_boxes_visible = are_boxes_visible(&world);
        if event.released(KeyCode::MouseLeft) {
            let params = MouseReleaseParams {
                prev_pos: self.prev_pos.clone(),
                visible: are_boxes_visible,
                is_alt_held: event.held_alt(),
                is_shift_held: event.held_shift(),
                is_ctrl_held: event.held_ctrl(),
            };
            (world, history, self.prev_pos) =
                on_mouse_released_left(event.mouse_pos, params, world, history);
        } else if event.released(KeyCode::MouseRight) {
            (world, history, self.prev_pos) = on_mouse_released_right(
                event.mouse_pos,
                self.prev_pos.clone(),
                are_boxes_visible,
                world,
                history,
            );
        } else {
            history.push(Record::new(world.data.clone(), ACTOR_NAME))
        }
        (world, history)
    }

    fn key_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        // up, down, left, right
        let shape_orig = world.data.shape();
        let split_mode = get_options(&world).map(|o| o.split_mode);
        let annos = get_annos_mut(&mut world);
        if let (Some(annos), Some(split_mode)) = (annos, split_mode) {
            if events.held(KeyCode::Up) && events.held_ctrl() {
                *annos = mem::take(annos).shift_min_bbs(0, -1, shape_orig, split_mode);
            } else if events.held(KeyCode::Down) && events.held_ctrl() {
                *annos = mem::take(annos).shift_min_bbs(0, 1, shape_orig, split_mode);
            } else if events.held(KeyCode::Right) && events.held_ctrl() {
                *annos = mem::take(annos).shift_min_bbs(1, 0, shape_orig, split_mode);
            } else if events.held(KeyCode::Left) && events.held_ctrl() {
                *annos = mem::take(annos).shift_min_bbs(-1, 0, shape_orig, split_mode);
            } else if events.held(KeyCode::Up) && events.held_alt() {
                *annos = mem::take(annos).shift(0, -1, shape_orig, split_mode);
            } else if events.held(KeyCode::Down) && events.held_alt() {
                *annos = mem::take(annos).shift(0, 1, shape_orig, split_mode);
            } else if events.held(KeyCode::Right) && events.held_alt() {
                *annos = mem::take(annos).shift(1, 0, shape_orig, split_mode);
            } else if events.held(KeyCode::Left) && events.held_alt() {
                *annos = mem::take(annos).shift(-1, 0, shape_orig, split_mode);
            } else if events.held(KeyCode::Up) {
                *annos = mem::take(annos).shift_max_bbs(0, -1, shape_orig, split_mode);
            } else if events.held(KeyCode::Down) {
                *annos = mem::take(annos).shift_max_bbs(0, 1, shape_orig, split_mode);
            } else if events.held(KeyCode::Right) {
                *annos = mem::take(annos).shift_max_bbs(1, 0, shape_orig, split_mode);
            } else if events.held(KeyCode::Left) {
                *annos = mem::take(annos).shift_max_bbs(-1, 0, shape_orig, split_mode);
            }
        }
        let are_boxes_visible = are_boxes_visible(&world);
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        (world, history)
    }

    fn key_released(
        &mut self,
        events: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let params = KeyReleasedParams {
            is_ctrl_held: events.held_ctrl(),
            released_key: map_released_key(events),
        };
        (world, history) = on_key_released(world, history, events.mouse_pos, params);
        (world, history)
    }
}

impl Manipulate for Bbox {
    fn new() -> Self {
        Self {
            prev_pos: PrevPos::default(),
            mover: Mover::new(),
        }
    }

    fn on_activate(&mut self, mut world: World, mut history: History) -> (World, History) {
        self.prev_pos = PrevPos::default();
        if let Some(data) = trace_ok(get_data_mut(&mut world)) {
            data.menu_active = true;
        }
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        let are_boxes_visible = true;
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        (world, history)
    }

    fn on_deactivate(&mut self, mut world: World, history: History) -> (World, History) {
        self.prev_pos = PrevPos::default();
        if let Some(td) = world.data.tools_data_map.get_mut(BBOX_NAME) {
            td.menu_active = false;
        }
        let are_boxes_visible = false;
        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
        (world, history)
    }

    fn on_filechange(&mut self, mut world: World, mut history: History) -> (World, History) {
        let options = get_options(&world);
        let bbox_data = get_specific_mut(&mut world);
        if let (Some(bbox_data), Some(options)) = (bbox_data, options) {
            for (_, (anno, _)) in bbox_data.anno_iter_mut() {
                anno.deselect_all();
            }
            (world, history) = check_autopaste(world, history, options.auto_paste);
            world.request_redraw_annotations(BBOX_NAME, options.core_options.visible);
        }
        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        mut history: History,
        events: &Events,
    ) -> (World, History) {
        world = check_recolorboxes(world);
        world = check_annoremove(world);

        world = check_cocoexport(world);

        world = check_cocoimport(world);

        let options = get_options(&world);

        if let Some(options) = options {
            world = check_trigger_redraw(world, BBOX_NAME, |d| {
                bbox_mut(d).map(|d| &mut d.options.core_options)
            });

            let in_menu_selected_label = current_cat_idx(&world);
            if let (Some(in_menu_selected_label), Some(mp)) =
                (in_menu_selected_label, events.mouse_pos)
            {
                if !self.prev_pos.prev_pos.is_empty() {
                    let geo = if self.prev_pos.prev_pos.len() == 1 {
                        GeoFig::BB(BB::from_points(mp.into(), self.prev_pos.prev_pos[0].into()))
                    } else {
                        GeoFig::Poly(
                            Polygon::from_vec(
                                self.prev_pos
                                    .prev_pos
                                    .iter()
                                    .chain(iter::once(&mp))
                                    .map(|p| (*p).into())
                                    .collect::<Vec<_>>(),
                            )
                            .unwrap(),
                        )
                    };
                    // animation
                    let label_info = get_specific(&world).map(|d| &d.label_info);

                    if let Some(label_info) = label_info {
                        let label = Some(label_info.labels()[in_menu_selected_label].clone());
                        let color = label_info.colors()[in_menu_selected_label];
                        let anno = BboxAnnotation {
                            geofig: geo,
                            label,
                            fill_color: Some(color),
                            fill_alpha: options.fill_alpha,
                            outline: Stroke {
                                color,
                                thickness: options.outline_thickness as f32 / 4.0,
                            },
                            outline_alpha: options.outline_alpha,
                            is_selected: None,
                        };
                        let are_boxes_visible = are_boxes_visible(&world);
                        world.request_redraw_annotations(BBOX_NAME, are_boxes_visible);
                        world.request_redraw_tmp_anno(Annotation::Bbox(anno));
                    }
                }
            }
            (world, history) = make_tool_transform!(
                self,
                world,
                history,
                events,
                [
                    (pressed, KeyCode::MouseRight, mouse_pressed),
                    (held, KeyCode::MouseRight, mouse_held),
                    (released, KeyCode::MouseLeft, mouse_released),
                    (released, KeyCode::MouseRight, mouse_released),
                    (released, KeyCode::Delete, key_released),
                    (released, KeyCode::Back, key_released),
                    (released, KeyCode::H, key_released),
                    (released, KeyCode::A, key_released),
                    (released, KeyCode::D, key_released),
                    (released, KeyCode::C, key_released),
                    (released, KeyCode::V, key_released),
                    (released, KeyCode::L, key_released),
                    (released, KeyCode::Down, key_released),
                    (released, KeyCode::Up, key_released),
                    (released, KeyCode::Left, key_released),
                    (released, KeyCode::Right, key_released),
                    (released, KeyCode::Key1, key_released),
                    (released, KeyCode::Key2, key_released),
                    (released, KeyCode::Key3, key_released),
                    (released, KeyCode::Key4, key_released),
                    (released, KeyCode::Key5, key_released),
                    (released, KeyCode::Key6, key_released),
                    (released, KeyCode::Key7, key_released),
                    (released, KeyCode::Key8, key_released),
                    (released, KeyCode::Key9, key_released),
                    (held, KeyCode::Down, key_held),
                    (held, KeyCode::Up, key_held),
                    (held, KeyCode::Left, key_held),
                    (held, KeyCode::Right, key_held)
                ]
            );
        }
        (world, history)
    }
}
