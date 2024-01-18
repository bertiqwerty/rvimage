use crate::{
    annotations_accessor, annotations_accessor_mut,
    domain::{shape_unscaled, BbF, Circle, ShapeI, TPtF},
    drawme::{Annotation, BboxAnnotation, Stroke},
    events::{Events, KeyCode},
    file_util,
    history::{History, Record},
    make_tool_transform,
    result::trace_ok,
    tools::{
        core::{
            check_erase_mode, check_recolorboxes, check_trigger_history_update,
            check_trigger_redraw, deselect_all, map_released_key, Mover,
        },
        rot90, Manipulate, BBOX_NAME,
    },
    tools_data::{
        self,
        annotations::BboxAnnotations,
        bbox_data::{self, ImportMode},
        bbox_mut, merge, vis_from_lfoption, LabelInfo, Rot90ToolData, OUTLINE_THICKNESS_CONVERSION,
    },
    tools_data_accessors, tools_data_accessors_objects,
    util::Visibility,
    world::World,
    GeoFig, Polygon,
};
use std::{iter, mem, time::Instant};

use super::on_events::{
    change_annos_bbox, export_if_triggered, find_close_vertex, import_coco_if_triggered,
    move_corner_tol, on_key_released, on_mouse_held_left, on_mouse_held_right,
    on_mouse_released_left, on_mouse_released_right, KeyReleasedParams, MouseHeldLeftParams,
    MouseMoveParams, MouseReleaseParams, PrevPos,
};
pub const ACTOR_NAME: &str = "Bbox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";
const MISSING_DATA_MSG: &str = "bbox tools data not available";
annotations_accessor_mut!(ACTOR_NAME, bbox_mut, MISSING_ANNO_MSG, BboxAnnotations);
annotations_accessor!(ACTOR_NAME, bbox, MISSING_ANNO_MSG, BboxAnnotations);
tools_data_accessors!(
    ACTOR_NAME,
    MISSING_DATA_MSG,
    bbox_data,
    BboxSpecificData,
    bbox,
    bbox_mut
);
tools_data_accessors_objects!(
    ACTOR_NAME,
    MISSING_DATA_MSG,
    bbox_data,
    BboxSpecificData,
    bbox,
    bbox_mut
);

pub(super) fn paste(mut world: World, mut history: History) -> (World, History) {
    let clipboard = get_specific(&world).and_then(|d| d.clipboard.clone());
    if let Some(clipboard) = &clipboard {
        let cb_bbs = clipboard.elts();
        if !cb_bbs.is_empty() {
            let shape_orig = ShapeI::from_im(world.data.im_background());
            let paste_annos = |a: &mut BboxAnnotations| {
                a.extend(
                    cb_bbs.iter().cloned(),
                    clipboard.cat_idxs().iter().copied(),
                    shape_orig,
                )
            };
            change_annos_bbox(&mut world, paste_annos);
        }
        set_visible(&mut world);
        history.push(Record::new(world.clone(), ACTOR_NAME));
    }

    (world, history)
}

pub(super) fn current_cat_idx(world: &World) -> Option<usize> {
    get_specific(world).map(|d| d.label_info.cat_idx_current)
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
        let data = get_specific_mut(&mut world);
        if let (Some(data), Some(opened_folder)) = (data, &opened_folder) {
            data.retain_fileannos_in_folder(opened_folder);
            data.options.is_anno_rm_triggered = false;
        }
        set_visible(&mut world);
    }
    world
}

fn get_rot90_data(world: &World) -> Option<&Rot90ToolData> {
    tools_data::get(world, rot90::ACTOR_NAME, "no rotation_data_found")
        .and_then(|d| d.specifics.rot90())
        .ok()
}

fn check_cocoexport(mut world: World) -> World {
    // export label file if demanded
    let bbox_data = get_specific(&world);
    if let Some(bbox_data) = bbox_data {
        let rot90_data = get_rot90_data(&world);
        export_if_triggered(&world.data.meta_data, bbox_data, rot90_data);
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
        let rot90_data = get_rot90_data(&world);
        if let Some(imported_data) = import_coco_if_triggered(
            &world.data.meta_data,
            if options.is_import_triggered {
                get_specific(&world).map(|o| &o.coco_file)
            } else {
                None
            },
            rot90_data,
        ) {
            if let Some(data_mut) = get_specific_mut(&mut world) {
                if options.is_import_triggered {
                    match options.import_mode {
                        ImportMode::Replace => {
                            data_mut.annotations_map = imported_data.annotations_map;
                            data_mut.label_info = imported_data.label_info;
                        }
                        ImportMode::Merge => {
                            let (annotations_map, label_info) = merge(
                                mem::take(&mut data_mut.annotations_map),
                                mem::take(&mut data_mut.label_info),
                                imported_data.annotations_map,
                                imported_data.label_info,
                            );
                            data_mut.annotations_map = annotations_map;
                            data_mut.label_info = label_info;
                        }
                    }
                    data_mut.options.is_import_triggered = false;
                }
                set_visible(&mut world);
            }
        } else if let Some(data_mut) = get_specific_mut(&mut world) {
            data_mut.options.is_import_triggered = false;
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
    start_press_time: Option<Instant>,
    points_at_press: Option<usize>,
    points_after_held: Option<usize>,
    last_close_circle_check: Option<Instant>,
}

impl Bbox {
    fn mouse_pressed(
        &mut self,
        event: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if get_options(&world).map(|o| o.core_options.erase) != Some(true) {
            if event.pressed(KeyCode::MouseRight) {
                self.mover.move_mouse_pressed(event.mouse_pos_on_orig);
            } else {
                self.start_press_time = Some(Instant::now());
                self.points_at_press = Some(self.prev_pos.prev_pos.len());
                if !(event.held_alt() || event.held_ctrl() || event.held_shift()) {
                    world = deselect_all(world, BBOX_NAME, get_annos_mut, get_label_info);
                }
            }
        }
        (world, history)
    }

    fn mouse_held(
        &mut self,
        event: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let params = MouseMoveParams {
            mover: &mut self.mover,
        };
        if event.held(KeyCode::MouseRight) {
            on_mouse_held_right(event.mouse_pos_on_orig, params, world, history)
        } else {
            let options = get_options(&world);
            let params = MouseHeldLeftParams {
                prev_pos: self.prev_pos.clone(),
                is_alt_held: event.held_alt(),
                is_shift_held: event.held_shift(),
                is_ctrl_held: event.held_ctrl(),
                distance: options.map(|o| o.drawing_distance).unwrap_or(2) as f64,
                elapsed_millis_since_press: self
                    .start_press_time
                    .map(|t| t.elapsed().as_millis())
                    .unwrap_or(0),
            };
            (world, history, self.prev_pos) =
                on_mouse_held_left(event.mouse_pos_on_orig, params, world, history);
            self.points_after_held = Some(self.prev_pos.prev_pos.len());
            (world, history)
        }
    }

    fn mouse_released(
        &mut self,
        event: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let close_box_or_poly = self.points_at_press.map(|x| x + 4) < self.points_after_held;
        let are_boxes_visible = get_visible(&world);
        if event.released(KeyCode::MouseLeft) {
            let params = MouseReleaseParams {
                prev_pos: self.prev_pos.clone(),
                visible: are_boxes_visible,
                is_alt_held: event.held_alt(),
                is_shift_held: event.held_shift(),
                is_ctrl_held: event.held_ctrl(),
                close_box_or_poly,
            };
            (world, history, self.prev_pos) =
                on_mouse_released_left(event.mouse_pos_on_orig, params, world, history);
        } else if event.released(KeyCode::MouseRight) {
            (world, history, self.prev_pos) = on_mouse_released_right(
                event.mouse_pos_on_orig,
                self.prev_pos.clone(),
                are_boxes_visible,
                world,
                history,
            );
        } else {
            history.push(Record::new(world.clone(), ACTOR_NAME))
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
        let shift_annos = |annos: &mut BboxAnnotations| {
            if let Some(split_mode) = split_mode {
                if events.held(KeyCode::Up) && events.held_ctrl() {
                    *annos = mem::take(annos).shift_min_bbs(0.0, -1.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Down) && events.held_ctrl() {
                    *annos = mem::take(annos).shift_min_bbs(0.0, 1.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Right) && events.held_ctrl() {
                    *annos = mem::take(annos).shift_min_bbs(1.0, 0.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Left) && events.held_ctrl() {
                    *annos = mem::take(annos).shift_min_bbs(-1.0, 0.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Up) && events.held_alt() {
                    *annos = mem::take(annos).shift(0.0, -1.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Down) && events.held_alt() {
                    *annos = mem::take(annos).shift(0.0, 1.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Right) && events.held_alt() {
                    *annos = mem::take(annos).shift(1.0, 0.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Left) && events.held_alt() {
                    *annos = mem::take(annos).shift(-1.0, 0.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Up) {
                    *annos = mem::take(annos).shift_max_bbs(0.0, -1.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Down) {
                    *annos = mem::take(annos).shift_max_bbs(0.0, 1.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Right) {
                    *annos = mem::take(annos).shift_max_bbs(1.0, 0.0, shape_orig, split_mode);
                } else if events.held(KeyCode::Left) {
                    *annos = mem::take(annos).shift_max_bbs(-1.0, 0.0, shape_orig, split_mode);
                }
            }
        };
        change_annos_bbox(&mut world, shift_annos);
        let vis = get_visible(&world);
        world.request_redraw_annotations(BBOX_NAME, vis);
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
        world = check_erase_mode(
            params.released_key,
            |w| get_options_mut(w).map(|o| &mut o.core_options),
            set_visible,
            world,
        );
        (world, history) = on_key_released(world, history, events.mouse_pos_on_orig, params);
        (world, history)
    }
}

impl Manipulate for Bbox {
    fn new() -> Self {
        Self {
            prev_pos: PrevPos::default(),
            mover: Mover::new(),
            start_press_time: None,
            points_after_held: None,
            points_at_press: None,
            last_close_circle_check: None,
        }
    }

    fn on_activate(&mut self, mut world: World) -> World {
        self.prev_pos = PrevPos::default();
        if let Some(data) = trace_ok(get_data_mut(&mut world)) {
            data.menu_active = true;
        }
        set_visible(&mut world);
        world
    }

    fn on_deactivate(&mut self, mut world: World) -> World {
        self.prev_pos = PrevPos::default();
        if let Some(td) = world.data.tools_data_map.get_mut(BBOX_NAME) {
            td.menu_active = false;
        }
        world.request_redraw_annotations(BBOX_NAME, Visibility::None);
        world
    }

    fn on_filechange(&mut self, mut world: World, mut history: History) -> (World, History) {
        let options = get_options(&world);
        let bbox_data = get_specific_mut(&mut world);
        if let (Some(bbox_data), Some(options)) = (bbox_data, options) {
            for (_, (anno, _)) in bbox_data.anno_iter_mut() {
                anno.deselect_all();
            }
            (world, history) = check_autopaste(world, history, options.auto_paste);
            let vis = get_visible(&world);
            world.request_redraw_annotations(BBOX_NAME, vis);
        }
        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        mut history: History,
        events: &Events,
    ) -> (World, History) {
        world = check_recolorboxes(
            world,
            BBOX_NAME,
            |world| get_options_mut(world).map(|o| &mut o.core_options),
            |world| get_specific_mut(world).map(|d| &mut d.label_info),
        );

        (world, history) = check_trigger_history_update(world, history, BBOX_NAME, |d| {
            bbox_mut(d).map(|d| &mut d.options.core_options)
        });
        world = check_annoremove(world);

        world = check_cocoexport(world);

        world = check_cocoimport(world);

        let options = get_options(&world);

        if let (Some(mp), Some(last_check)) =
            (events.mouse_pos_on_orig, self.last_close_circle_check)
        {
            if last_check.elapsed().as_millis() > 2 {
                let geos = get_annos_if_some(&world).map(|a| a.elts());
                if let Some((bb_idx, c_idx)) = geos.and_then(|geos| {
                    let unscaled = shape_unscaled(world.zoom_box(), world.shape_orig());
                    let tolerance = move_corner_tol(unscaled);
                    find_close_vertex(mp, geos, tolerance)
                }) {
                    let annos = get_annos(&world);
                    let corner_point = annos.map(|a| &a.elts()[bb_idx]).map(|a| a.point(c_idx));
                    let data = get_specific_mut(&mut world);
                    if let (Some(data), Some(corner_point), Some(options)) =
                        (data, corner_point, options)
                    {
                        data.highlight_circles = vec![Circle {
                            center: corner_point,
                            radius: options.outline_thickness as TPtF
                                / OUTLINE_THICKNESS_CONVERSION
                                * 2.5,
                        }];
                        let vis = get_visible(&world);
                        world.request_redraw_annotations(BBOX_NAME, vis);
                    }
                } else {
                    let data = get_specific_mut(&mut world);
                    let n_circles = data
                        .as_ref()
                        .map(|d| d.highlight_circles.len())
                        .unwrap_or(0);
                    if let Some(data) = data {
                        data.highlight_circles = vec![];
                    }
                    if n_circles > 0 {
                        let vis = get_visible(&world);
                        world.request_redraw_annotations(BBOX_NAME, vis);
                    }
                }
                self.last_close_circle_check = Some(Instant::now());
            }
        } else {
            self.last_close_circle_check = Some(Instant::now());
        }

        if let Some(options) = options {
            world = check_trigger_redraw(world, BBOX_NAME, get_label_info, |d| {
                bbox_mut(d).map(|d| &mut d.options.core_options)
            });

            let in_menu_selected_label = current_cat_idx(&world);
            if let (Some(in_menu_selected_label), Some(mp)) =
                (in_menu_selected_label, events.mouse_pos_on_orig)
            {
                if !self.prev_pos.prev_pos.is_empty() {
                    let geo = if self.prev_pos.prev_pos.len() == 1 {
                        GeoFig::BB(BbF::from_points(mp, self.prev_pos.prev_pos[0]))
                    } else {
                        GeoFig::Poly(
                            Polygon::from_vec(
                                self.prev_pos
                                    .prev_pos
                                    .iter()
                                    .chain(iter::once(&mp))
                                    .copied()
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
                            fill_alpha: 0,
                            outline: Stroke {
                                color,
                                thickness: options.outline_thickness as TPtF / 4.0,
                            },
                            outline_alpha: options.outline_alpha,
                            is_selected: None,
                            highlight_circles: vec![],
                        };
                        let vis = get_visible(&world);
                        world.request_redraw_annotations(BBOX_NAME, vis);
                        world.request_redraw_tmp_anno(Annotation::Bbox(anno));
                    }
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
                (pressed, KeyCode::MouseLeft, mouse_pressed),
                (held, KeyCode::MouseRight, mouse_held),
                (held, KeyCode::MouseLeft, mouse_held),
                (released, KeyCode::MouseLeft, mouse_released),
                (released, KeyCode::MouseRight, mouse_released),
                (released, KeyCode::Delete, key_released),
                (released, KeyCode::Back, key_released),
                (released, KeyCode::H, key_released),
                (released, KeyCode::A, key_released),
                (released, KeyCode::D, key_released),
                (released, KeyCode::E, key_released),
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
        (world, history)
    }
}

#[cfg(test)]
use {
    super::on_events::test_data,
    crate::cfg::{ExportPath, ExportPathConnection},
    crate::Event,
    std::{path::PathBuf, thread, time::Duration},
};
#[test]
fn test_bbox_ctrl_h() {
    let (_, mut world, mut history) = test_data();
    let mut bbox = Bbox::new();
    bbox.last_close_circle_check = Some(Instant::now());
    thread::sleep(Duration::from_millis(3));
    assert_eq!(get_visible(&world), Visibility::All);
    let events = Events::default()
        .events(vec![
            Event::Held(KeyCode::Ctrl),
            Event::Released(KeyCode::H),
        ])
        .mousepos_orig(Some((1.0, 1.0).into()));
    (world, history) = bbox.events_tf(world, history, &events);
    thread::sleep(Duration::from_millis(3));
    (world, _) = bbox.events_tf(
        world,
        history,
        &Events::default().mousepos_orig(Some((1.0, 1.0).into())),
    );
    assert_eq!(get_visible(&world), Visibility::None);
}

#[test]
fn test_coco_import_label_info() {
    const TEST_DATA_FOLDER: &str = "resources/test_data/";
    let (_, mut world, history) = test_data();
    let data = get_specific_mut(&mut world).unwrap();
    data.coco_file = ExportPath {
        path: PathBuf::from(format!("{}catids_12_coco.json", TEST_DATA_FOLDER)),
        conn: ExportPathConnection::Local,
    };
    let label_info_before = data.label_info.clone();
    data.options.is_import_triggered = true;
    let mut bbox = Bbox::new();
    let events = Events::default();
    (world, _) = bbox.events_tf(world, history, &events);
    let data = get_specific(&world).unwrap();
    let label_info_after = data.label_info.clone();
    assert_eq!(label_info_before.labels(), &["foreground", "label"]);
    assert_eq!(label_info_after.labels(), &["first label", "second label"]);
    assert!(!data.options.is_import_triggered);
}
