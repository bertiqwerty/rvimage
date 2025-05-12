use crate::{
    annotations_accessor_mut,
    drawme::{Annotation, BboxAnnotation, Stroke},
    events::{Events, KeyCode},
    history::{History, Record},
    instance_annotations_accessor, make_tool_transform,
    result::trace_ok_err,
    tools::{
        core::{
            check_autopaste, check_erase_mode, check_recolorboxes, check_trigger_history_update,
            check_trigger_redraw, deselect_all, instance_label_display_sort, map_released_key,
            Mover,
        },
        instance_anno_shared::{check_cocoimport, get_rot90_data},
        Manipulate, BBOX_NAME,
    },
    tools_data::{
        annotations::BboxAnnotations, bbox_data, vis_from_lfoption, LabelInfo,
        OUTLINE_THICKNESS_CONVERSION,
    },
    tools_data_accessors, tools_data_accessors_objects,
    util::Visibility,
    world::World,
    world_annotations_accessor, GeoFig, Polygon,
};
use rvimage_domain::{shape_unscaled, BbF, Circle, PtF, TPtF};
use std::{iter, mem, time::Instant};

use super::on_events::{
    change_annos_bbox, closest_corner, export_if_triggered, find_close_vertex, import_coco,
    move_corner_tol, on_key_released, on_mouse_held_left, on_mouse_held_right,
    on_mouse_released_left, on_mouse_released_right, KeyReleasedParams, MouseHeldLeftParams,
    MouseReleaseParams, PrevPos,
};
pub const ACTOR_NAME: &str = "Bbox";
const MISSING_ANNO_MSG: &str = "bbox annotations have not yet been initialized";
const MISSING_DATA_MSG: &str = "bbox tools data not available";
annotations_accessor_mut!(ACTOR_NAME, bbox_mut, MISSING_ANNO_MSG, BboxAnnotations);
world_annotations_accessor!(ACTOR_NAME, bbox, MISSING_ANNO_MSG, BboxAnnotations);
instance_annotations_accessor!(GeoFig);
tools_data_accessors!(
    ACTOR_NAME,
    MISSING_DATA_MSG,
    bbox_data,
    BboxToolData,
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

pub(super) fn current_cat_idx(world: &World) -> Option<usize> {
    get_specific(world).map(|d| d.label_info.cat_idx_current)
}

fn check_cocoexport(mut world: World) -> World {
    // export label file if demanded
    let bbox_data = get_specific(&world);
    if let Some(bbox_data) = bbox_data {
        let rot90_data = get_rot90_data(&world);
        export_if_triggered(&world.data.meta_data, bbox_data, rot90_data);
        if let Some(o) = get_options_mut(&mut world) {
            o.core.import_export_trigger.untrigger_export();
        }
    }
    world
}

fn show_grab_ball(
    mp: Option<PtF>,
    prev_pos: &PrevPos,
    world: &mut World,
    last_proximal_circle_check: Option<Instant>,
    options: Option<&bbox_data::Options>,
) -> Instant {
    if last_proximal_circle_check.map(|lc| lc.elapsed().as_millis()) > Some(2) {
        if let Some(mp) = mp {
            if prev_pos.prev_pos.is_empty() {
                let label_info = get_label_info(world);
                let geos = get_annos_if_some(world).map(|a| {
                    (0..a.elts().len())
                        .filter(|elt_idx| {
                            let cur = label_info.map(|li| li.cat_idx_current);
                            let show_only_current = label_info.map(|li| li.show_only_current);
                            a.is_of_current_label(*elt_idx, cur, show_only_current)
                        })
                        .map(|elt_idx| (elt_idx, &a.elts()[elt_idx]))
                });
                if let Some((bb_idx, c_idx)) = geos.and_then(|geos| {
                    let unscaled = shape_unscaled(world.zoom_box(), world.shape_orig());
                    let tolerance = move_corner_tol(unscaled);
                    find_close_vertex(mp, geos, tolerance)
                }) {
                    let annos = get_annos(world);
                    let corner_point = annos.map(|a| &a.elts()[bb_idx]).map(|a| a.point(c_idx));
                    let data = get_specific_mut(world);
                    if let (Some(data), Some(corner_point), Some(options)) =
                        (data, corner_point, options)
                    {
                        data.highlight_circles = vec![Circle {
                            center: corner_point,
                            radius: TPtF::from(options.outline_thickness)
                                / OUTLINE_THICKNESS_CONVERSION
                                * 2.5,
                        }];
                        let vis = get_visible(world);
                        world.request_redraw_annotations(BBOX_NAME, vis);
                    }
                } else {
                    let data = get_specific_mut(world);
                    let n_circles = data.as_ref().map_or(0, |d| d.highlight_circles.len());
                    if let Some(data) = data {
                        data.highlight_circles = vec![];
                    }
                    if n_circles > 0 {
                        let vis = get_visible(world);
                        world.request_redraw_annotations(BBOX_NAME, vis);
                    }
                }
            } else {
                let (c_idx, c_dist) = closest_corner(mp, prev_pos.prev_pos.iter().copied());
                let unscaled = shape_unscaled(world.zoom_box(), world.shape_orig());
                let tolerance = move_corner_tol(unscaled);
                if c_dist < tolerance {
                    let center = prev_pos.prev_pos[c_idx];
                    let data = get_specific_mut(world);
                    if let (Some(data), Some(options)) = (data, options) {
                        data.highlight_circles = vec![Circle {
                            center,
                            radius: TPtF::from(options.outline_thickness)
                                / OUTLINE_THICKNESS_CONVERSION
                                * 3.5,
                        }];
                        let vis = get_visible(world);
                        world.request_redraw_annotations(BBOX_NAME, vis);
                    }
                } else {
                    let data = get_specific_mut(world);
                    if let Some(data) = data {
                        data.highlight_circles = vec![];
                    }
                    let vis = get_visible(world);
                    world.request_redraw_annotations(BBOX_NAME, vis);
                }
            }
        }
    }
    Instant::now()
}

#[derive(Clone, Debug)]
pub struct Bbox {
    prev_pos: PrevPos,
    mover: Mover,
    start_press_time: Option<Instant>,
    points_at_press: Option<usize>,
    points_after_held: Option<usize>,
    last_proximal_circle_check: Option<Instant>,
}

impl Bbox {
    fn mouse_pressed(
        &mut self,
        event: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if get_options(&world).map(|o| o.core.erase) != Some(true) {
            if event.pressed(KeyCode::MouseRight) {
                self.mover.move_mouse_pressed(event.mouse_pos_on_orig);
            } else {
                self.start_press_time = Some(Instant::now());
                self.points_at_press = Some(self.prev_pos.prev_pos.len());
                if !(event.held_alt() || event.held_ctrl() || event.held_shift()) {
                    world =
                        deselect_all::<_, DataAccessors, InstanceAnnoAccessors>(world, BBOX_NAME);
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
        if event.held(KeyCode::MouseRight) {
            on_mouse_held_right(event.mouse_pos_on_orig, &mut self.mover, world, history)
        } else {
            let options = get_options(&world);
            let params = MouseHeldLeftParams {
                prev_pos: self.prev_pos.clone(),
                is_alt_held: event.held_alt(),
                is_shift_held: event.held_shift(),
                is_ctrl_held: event.held_ctrl(),
                distance: f64::from(options.map_or(2, |o| o.drawing_distance)),
                elapsed_millis_since_press: self
                    .start_press_time
                    .map_or(0, |t| t.elapsed().as_millis()),
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
        // evaluate if a box or a polygon should be closed based on the number of points
        // at the time of the press and the number of points after the held
        let close_box_or_poly = self.points_at_press.map(|x| x + 4) < self.points_after_held;
        self.points_at_press = None;
        self.points_after_held = None;

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
            history.push(Record::new(world.clone(), ACTOR_NAME));
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
        world = check_erase_mode::<DataAccessors>(params.released_key, set_visible, world);
        (world, history) = on_key_released(world, history, events.mouse_pos_on_orig, &params);
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
            last_proximal_circle_check: None,
        }
    }

    fn on_activate(&mut self, mut world: World) -> World {
        self.prev_pos = PrevPos::default();
        if let Some(data) = trace_ok_err(get_data_mut(&mut world)) {
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
    fn on_always_active_zoom(&mut self, mut world: World, history: History) -> (World, History) {
        let visible = get_options(&world).map(|o| o.core.visible) == Some(true);
        let vis = vis_from_lfoption(get_label_info(&world), visible);
        world.request_redraw_annotations(BBOX_NAME, vis);
        (world, history)
    }
    fn on_filechange(&mut self, mut world: World, mut history: History) -> (World, History) {

        use_currentimageshape_for_annos(&mut world);

        let bbox_data = get_specific_mut(&mut world);
        if let Some(bbox_data) = bbox_data {
            for (_, (anno, _)) in bbox_data.anno_iter_mut() {
                anno.deselect_all();
            }
            let ild = get_instance_label_display(&world);
            world = instance_label_display_sort::<_, DataAccessors, InstanceAnnoAccessors>(
                world, ild, ACTOR_NAME,
            );
        }

        let visible = get_options(&world).map(|o| o.core.visible) == Some(true);
        let vis = vis_from_lfoption(get_label_info(&world), visible);
        world.request_redraw_annotations(BBOX_NAME, vis);

        (world, history) =
            check_autopaste::<_, DataAccessors, InstanceAnnoAccessors>(world, history, ACTOR_NAME);

        (world, history)
    }

    fn events_tf(
        &mut self,
        mut world: World,
        mut history: History,
        events: &Events,
    ) -> (World, History) {
        world = check_recolorboxes::<DataAccessors>(world, BBOX_NAME);

        (world, history) = check_trigger_history_update::<DataAccessors>(world, history, BBOX_NAME);

        world = check_cocoexport(world);
        let imported;
        (world, imported) = check_cocoimport::<_, _, DataAccessors>(
            world,
            get_specific,
            get_specific_mut,
            import_coco,
        );
        if imported {
            set_visible(&mut world);
        }

        let options = get_options(&world).copied();

        self.last_proximal_circle_check = Some(show_grab_ball(
            events.mouse_pos_on_orig,
            &self.prev_pos,
            &mut world,
            self.last_proximal_circle_check,
            options.as_ref(),
        ));
        if let Some(options) = options {
            world = check_trigger_redraw::<DataAccessors>(world, BBOX_NAME);

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
                    let circles = get_specific(&world).map(|d| d.highlight_circles.clone());
                    let label_info = get_specific(&world).map(|d| &d.label_info);

                    if let (Some(circles), Some(label_info)) = (circles, label_info) {
                        let label = Some(label_info.labels()[in_menu_selected_label].clone());
                        let color = label_info.colors()[in_menu_selected_label];
                        let anno = BboxAnnotation {
                            geofig: geo,
                            label,
                            fill_color: Some(color),
                            fill_alpha: 0,
                            outline: Stroke {
                                color,
                                thickness: TPtF::from(options.outline_thickness) / 4.0,
                            },
                            outline_alpha: options.outline_alpha,
                            is_selected: None,
                            highlight_circles: circles,
                            instance_label_display: options.core.instance_label_display,
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
    bbox.last_proximal_circle_check = Some(Instant::now());
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
    data.options.core.import_export_trigger.trigger_import();
    let mut bbox = Bbox::new();
    let events = Events::default();
    let (mut world, history) = bbox.events_tf(world, history, &events);
    let data = get_specific(&world).unwrap();
    assert_eq!(label_info_before.labels(), &["rvimage_fg", "label"]);
    assert_eq!(label_info_before.cat_ids(), &[1, 2]);
    assert_eq!(data.label_info.labels(), &["first label", "second label"]);
    assert_eq!(data.label_info.cat_ids(), &[1, 2]);
    assert!(!data.options.core.import_export_trigger.import_triggered());

    // now we import another coco file with different labels
    let data = get_specific_mut(&mut world).unwrap();
    data.coco_file = ExportPath {
        path: PathBuf::from(format!("{}catids_01_coco_3labels.json", TEST_DATA_FOLDER)),
        conn: ExportPathConnection::Local,
    };
    data.options.core.import_export_trigger.trigger_import();
    let (world, _) = bbox.events_tf(world, history, &events);
    let data = get_specific(&world).unwrap();
    assert_eq!(
        data.label_info.labels(),
        &["first label", "second label", "third label"]
    );
    assert_eq!(data.label_info.cat_ids(), &[0, 1, 2]);
    let all_occurring_cats = data
        .annotations_map
        .iter()
        .flat_map(|(_, (v, _))| v.cat_idxs().iter().copied())
        .collect::<Vec<usize>>();
    assert!(all_occurring_cats.contains(&0));
    assert!(all_occurring_cats.contains(&1));
    assert!(all_occurring_cats.contains(&2));
}
