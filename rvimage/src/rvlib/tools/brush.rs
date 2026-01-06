use brush_data::BrushToolData;
use std::{cmp::Ordering, mem, sync::mpsc::Receiver, thread};

use super::{
    BRUSH_NAME, Manipulate,
    core::{
        HeldKey, Mover, ReleasedKey, change_annos, check_autopaste, check_erase_mode,
        check_instance_label_display_change, deselect_all, instance_label_display_sort,
        label_change_key, map_held_key, map_released_key, on_selection_keys,
    },
    instance_anno_shared::get_rot90_data,
};
use crate::{
    Annotation, BrushAnnotation, Line, ShapeI, annotations_accessor_mut,
    cfg::ExportPath,
    events::{Events, KeyCode},
    history::{History, Record},
    instance_annotations_accessor, make_tool_transform,
    meta_data::MetaData,
    result::trace_ok_err,
    tools::{
        core::{check_recolorboxes, check_trigger_history_update, check_trigger_redraw},
        instance_anno_shared::{check_cocoimport, predictive_labeling},
    },
    tools_data::{
        self, ExportAsCoco, InstanceAnnotate, LabelInfo, Rot90ToolData,
        annotations::{BrushAnnotations, InstanceAnnotations},
        brush_data::{self, MAX_INTENSITY, MAX_THICKNESS, MIN_INTENSITY, MIN_THICKNESS},
        coco_io::to_per_file_crowd,
        vis_from_lfoption,
    },
    tools_data_accessors, tools_data_accessors_objects,
    util::Visibility,
    world::World,
    world_annotations_accessor,
};
use rvimage_domain::{BrushLine, Canvas, PtF, TPtF};

pub const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";
const MISSING_DATA_MSG: &str = "brush data not available";
annotations_accessor_mut!(ACTOR_NAME, brush_mut, MISSING_ANNO_MSG, BrushAnnotations);
world_annotations_accessor!(ACTOR_NAME, brush, MISSING_ANNO_MSG, BrushAnnotations);
instance_annotations_accessor!(Canvas);
tools_data_accessors!(
    ACTOR_NAME,
    MISSING_DATA_MSG,
    brush_data,
    BrushToolData,
    brush,
    brush_mut
);
tools_data_accessors_objects!(
    ACTOR_NAME,
    MISSING_DATA_MSG,
    brush_data,
    BrushToolData,
    brush,
    brush_mut
);
pub(super) fn change_annos_brush(world: &mut World, change: impl FnOnce(&mut BrushAnnotations)) {
    change_annos::<_, DataAccessors, InstanceAnnoAccessors>(world, change);
}

fn import_coco(
    meta_data: &MetaData,
    coco_file: &ExportPath,
    rot90_data: Option<&Rot90ToolData>,
) -> Option<BrushToolData> {
    trace_ok_err(tools_data::coco_io::read_coco(meta_data, coco_file, rot90_data).map(|(_, d)| d))
}

fn max_select_dist(shape: ShapeI) -> TPtF {
    (TPtF::from(shape.w.pow(2) + shape.h.pow(2)).sqrt() / 100.0).max(50.0)
}

fn draw_erase_circle(mut world: World, mp: PtF) -> World {
    let show_only_current = get_specific(&world).map(|d| d.label_info.show_only_current);
    let options = get_options(&world).copied();
    let idx_current = get_specific(&world).map(|d| d.label_info.cat_idx_current);
    if let Some(options) = options {
        let erase = |annos: &mut BrushAnnotations| {
            let to_be_removed_line_idx = find_closest_canvas(annos, mp, |idx| {
                annos.is_of_current_label(idx, idx_current, show_only_current)
            });
            if let Some((idx, _)) = to_be_removed_line_idx {
                let canvas = annos.edit(idx);
                trace_ok_err(canvas.draw_circle(mp, options.thickness, 0));
            }
        };
        change_annos_brush(&mut world, erase);
        set_visible(&mut world);
    }
    world
}
fn mouse_released(events: &Events, mut world: World, mut history: History) -> (World, History) {
    if events.held_ctrl() {
        let shape_orig = world.shape_orig();
        let show_only_current = get_specific(&world).map(|d| d.label_info.show_only_current);
        let idx_current = get_specific(&world).map(|d| d.label_info.cat_idx_current);
        if let (Some(mp), Some(annos)) = (events.mouse_pos_on_orig, get_annos_mut(&mut world)) {
            let to_be_selected_line_idx = find_closest_canvas(annos, mp, |idx| {
                annos.is_of_current_label(idx, idx_current, show_only_current)
            });
            if let Some((idx, dist)) = to_be_selected_line_idx {
                if dist < max_select_dist(shape_orig) {
                    if annos.selected_mask()[idx] {
                        annos.deselect(idx);
                    } else {
                        annos.select(idx);
                    }
                } else {
                    world =
                        deselect_all::<_, DataAccessors, InstanceAnnoAccessors>(world, BRUSH_NAME);
                }
            }
        }
        set_visible(&mut world);
    } else if !(events.held_alt() || events.held_shift()) {
        // neither shift nor alt nor ctrl were held => a brushline has been finished
        // or a brush line has been deleted.
        let erase = get_options(&world).map(|o| o.core.erase);
        let cat_idx = get_specific(&world).map(|o| o.label_info.cat_idx_current);
        if erase != Some(true) {
            let shape_orig = world.shape_orig();
            let line = get_specific_mut(&mut world).and_then(|d| mem::take(&mut d.tmp_line));
            let line = if let Some((line, _)) = line {
                Some(line)
            } else if let (Some(mp), Some(options)) =
                (events.mouse_pos_on_orig, get_options(&world))
            {
                Some(BrushLine {
                    line: Line::from(mp),
                    intensity: options.intensity,
                    thickness: options.thickness,
                })
            } else {
                None
            };
            let ild = get_instance_label_display(&world);

            let change_annos = |annos: &mut BrushAnnotations| {
                if let (Some(line), Some(cat_idx)) = (line, cat_idx) {
                    let canvas = Canvas::new(&line, shape_orig, None);
                    if let Ok(canvas) = canvas {
                        annos.add_elt(canvas, cat_idx, ild);
                    }
                }
            };
            change_annos_brush(&mut world, change_annos);
            set_visible(&mut world);
        } else if let Some(mp) = events.mouse_pos_on_orig {
            world = draw_erase_circle(world, mp);
        }
        history.push(Record::new(world.clone(), ACTOR_NAME));
    }
    (world, history)
}
fn mouse_pressed_left(events: &Events, mut world: World) -> World {
    if !(events.held_alt() || events.held_ctrl() || events.held_shift()) {
        world = deselect_all::<_, DataAccessors, InstanceAnnoAccessors>(world, BRUSH_NAME);
    }
    if !events.held_ctrl() {
        let options = get_options(&world).copied();
        let idx_current = get_specific(&world).map(|d| d.label_info.cat_idx_current);
        if let (Some(mp), Some(options)) = (events.mouse_pos_on_orig, options) {
            let erase = options.core.erase;
            if !erase && let (Some(d), Some(cat_idx)) = (get_specific_mut(&mut world), idx_current)
            {
                let line = Line::from(mp);
                d.tmp_line = Some((
                    BrushLine {
                        line,
                        intensity: options.intensity,
                        thickness: options.thickness,
                    },
                    cat_idx,
                ));
            }
        }
        set_visible(&mut world);
    }
    world
}
fn key_released(events: &Events, mut world: World, mut history: History) -> (World, History) {
    let released_key = map_released_key(events);
    (world, history) = on_selection_keys::<_, DataAccessors, InstanceAnnoAccessors>(
        world,
        history,
        released_key,
        events.held_ctrl(),
        BRUSH_NAME,
    );
    let mut trigger_redraw = false;
    if let Some(label_info) = get_specific_mut(&mut world).map(|s| &mut s.label_info) {
        (*label_info, trigger_redraw) = label_change_key(released_key, mem::take(label_info));
    }
    if trigger_redraw {
        let visible = get_options(&world).map(|o| o.core.visible) == Some(true);
        let vis = vis_from_lfoption(get_label_info(&world), visible);
        world.request_redraw_annotations(BRUSH_NAME, vis);
    }
    match released_key {
        ReleasedKey::H if events.held_ctrl() => {
            // Hide all boxes (selected or not)
            if let Some(options_mut) = get_options_mut(&mut world) {
                options_mut.core.visible = !options_mut.core.visible;
            }
            let vis = get_visible(&world);
            world.request_redraw_annotations(BRUSH_NAME, vis);
        }
        _ => (),
    }
    world = check_instance_label_display_change::<_, DataAccessors, InstanceAnnoAccessors>(
        world,
        released_key,
        ACTOR_NAME,
    );
    world = check_erase_mode::<DataAccessors>(released_key, set_visible, world);
    (world, history)
}
fn find_closest_canvas(
    annos: &BrushAnnotations,
    p: PtF,
    predicate: impl Fn(usize) -> bool,
) -> Option<(usize, f64)> {
    annos
        .elts()
        .iter()
        .enumerate()
        .map(|(i, cvs)| {
            (
                i,
                cvs.dist_to_boundary(p) * if cvs.contains(p) { 0.0 } else { 1.0 },
            )
        })
        .filter(|(i, _)| predicate(*i))
        .min_by(|(_, x), (_, y)| match x.partial_cmp(y) {
            Some(o) => o,
            None => Ordering::Greater,
        })
}

fn check_selected_intensity_thickness(mut world: World) -> World {
    let options = get_options(&world).copied();
    let annos = get_annos_mut(&mut world);
    let mut any_selected = false;
    if let (Some(annos), Some(options)) = (annos, options)
        && options.is_selection_change_needed
    {
        for brushline in annos.selected_elts_iter_mut() {
            brushline.intensity = options.intensity;
            any_selected = true;
        }
    }
    let options_mut = get_options_mut(&mut world);
    if let Some(options_mut) = options_mut {
        options_mut.is_selection_change_needed = false;
        if any_selected {
            options_mut.core.is_redraw_annos_triggered = true;
        }
    }
    world
}

fn check_export(mut world: World) -> World {
    let options = get_options(&world);
    let specifics = get_specific(&world);

    if options.map(|o| o.core.import_export_trigger.export_triggered()) == Some(true) {
        let rot90_data = get_rot90_data(&world).cloned();
        if let Some(data) = specifics {
            let meta_data = world.data.meta_data.clone();
            let mut data = data.clone();
            let per_file_crowd = options.map(|o| o.per_file_crowd) == Some(true);
            let double_check_shape =
                options.map(|o| o.core.doublecheck_cocoexport_shape) == Some(true);
            let f_export = move || {
                let start = std::time::Instant::now();
                if per_file_crowd {
                    to_per_file_crowd(&mut data.annotations_map);
                }
                let coco_file_conn = data.cocofile_conn();
                match tools_data::write_coco(
                    &meta_data,
                    data,
                    rot90_data.as_ref(),
                    &coco_file_conn,
                    double_check_shape,
                ) {
                    Ok((p, _)) => tracing::info!("export to {p:?} successfully triggered"),
                    Err(e) => tracing::error!("trigger export failed due to {e:?}"),
                };
                tracing::info!("export took {} seconds", start.elapsed().as_secs_f32());
            };
            thread::spawn(f_export);
        }
        if let Some(options_mut) = get_options_mut(&mut world) {
            options_mut.core.import_export_trigger.untrigger_export();
        }
    }
    world
}

pub(super) fn on_mouse_held_right(
    mouse_pos: Option<PtF>,
    mover: &mut Mover,
    mut world: World,
    history: History,
) -> (World, History) {
    if get_options(&world).map(|o| o.core.erase) != Some(true) {
        let orig_shape = world.data.shape();
        let move_boxes = |mpo_from, mpo_to| {
            let annos = get_annos_mut(&mut world);
            if let Some(annos) = annos {
                let (mut elts, cat_idxs, selected_mask) = mem::take(annos).separate_data();
                for (i, anno) in elts.iter_mut().enumerate() {
                    if selected_mask[i] {
                        anno.follow_movement(mpo_from, mpo_to, orig_shape);
                    }
                }
                *annos = InstanceAnnotations::new(elts, cat_idxs, selected_mask).unwrap();
            }
            Some(())
        };
        mover.move_mouse_held(move_boxes, mouse_pos);
        let vis = get_visible(&world);
        world.request_redraw_annotations(ACTOR_NAME, vis);
    }
    (world, history)
}
#[derive(Debug)]
pub struct Brush {
    mover: Mover,
    prediction_receiver: Option<Receiver<(World, History)>>,
}

impl Brush {
    fn mouse_pressed(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if events.pressed(KeyCode::MouseRight) {
            self.mover.move_mouse_pressed(events.mouse_pos_on_orig);
        } else {
            world = mouse_pressed_left(events, world);
        }
        (world, history)
    }
    fn mouse_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if events.held(KeyCode::MouseRight) {
            on_mouse_held_right(events.mouse_pos_on_orig, &mut self.mover, world, history)
        } else {
            if !events.held_ctrl() {
                let options = get_options(&world).copied();
                if let (Some(mp), Some(options)) = (events.mouse_pos_on_orig, options) {
                    if options.core.erase {
                        world = draw_erase_circle(world, mp);
                    } else {
                        let line = if let Some((line, _)) =
                            get_specific_mut(&mut world).and_then(|d| d.tmp_line.as_mut())
                        {
                            let last_point = line.line.last_point();
                            let dist = if let Some(last_point) = last_point {
                                last_point.dist_square(&mp)
                            } else {
                                100.0
                            };
                            if dist >= 3.0 {
                                line.line.push(mp);
                            }
                            Some(line.clone())
                        } else {
                            None
                        };
                        if let (Some(line), Some(color)) = (
                            line,
                            get_specific(&world)
                                .map(|d| d.label_info.colors()[d.label_info.cat_idx_current]),
                        ) {
                            let orig_shape = world.shape_orig();
                            let canvas_with_new_buffer = || {
                                let lower_buffer_bound = 100;
                                let extension_factor = if line.line.points.len() < 10 {
                                    4.0
                                } else if line.line.points.len() < 50 {
                                    3.0
                                } else {
                                    2.0
                                };
                                Canvas::from_line_extended(
                                    &line,
                                    orig_shape,
                                    extension_factor,
                                    lower_buffer_bound,
                                )
                            };
                            let canvas = if let Some(buffer) =
                                mem::take(&mut world.update_view.tmp_anno_buffer)
                            {
                                match buffer {
                                    Annotation::Brush(brush_anno) => {
                                        tracing::debug!("found buffer for tmp anno");
                                        Canvas::new(&line, orig_shape, Some(brush_anno.canvas.mask))
                                    }
                                    _ => canvas_with_new_buffer(),
                                }
                            } else {
                                canvas_with_new_buffer()
                            };

                            let canvas = trace_ok_err(canvas);
                            if let Some(canvas) = canvas {
                                world.request_redraw_tmp_anno(Annotation::Brush(BrushAnnotation {
                                    canvas,
                                    color,
                                    label: None,
                                    is_selected: None,
                                    fill_alpha: options.fill_alpha,
                                    instance_display_label: options.core.instance_label_display,
                                }));
                            }
                        }
                    }
                }
            }

            (world, history)
        }
    }

    fn mouse_released(
        &mut self,
        events: &Events,
        world: World,
        history: History,
    ) -> (World, History) {
        mouse_released(events, world, history)
    }

    #[allow(clippy::unused_self)]
    fn key_released(
        &mut self,
        events: &Events,
        world: World,
        history: History,
    ) -> (World, History) {
        key_released(events, world, history)
    }
    fn key_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        const INTENSITY_STEP: f64 = MAX_INTENSITY / 20.0;
        const THICKNESS_STEP: f64 = MAX_THICKNESS / 20.0;
        let held_key = map_held_key(events);
        let snap_to_step = |x: TPtF, step: TPtF| {
            if x < 2.0 * step {
                (x.div_euclid(step)) * step
            } else {
                x
            }
        };
        match held_key {
            HeldKey::I if events.held_alt() => {
                if let Some(o) = get_options_mut(&mut world) {
                    o.intensity = MIN_INTENSITY
                        .max(snap_to_step(o.intensity - INTENSITY_STEP, INTENSITY_STEP));
                    o.is_selection_change_needed = true;
                }
            }
            HeldKey::I => {
                if let Some(o) = get_options_mut(&mut world) {
                    o.intensity = MAX_INTENSITY
                        .min(snap_to_step(o.intensity + INTENSITY_STEP, INTENSITY_STEP));
                    o.is_selection_change_needed = true;
                }
            }
            HeldKey::T if events.held_alt() => {
                if let Some(o) = get_options_mut(&mut world) {
                    o.thickness = MIN_THICKNESS
                        .max(snap_to_step(o.thickness - THICKNESS_STEP, THICKNESS_STEP));
                    o.is_selection_change_needed = true;
                }
            }
            HeldKey::T => {
                if let Some(o) = get_options_mut(&mut world) {
                    o.thickness = MAX_THICKNESS
                        .min(snap_to_step(o.thickness + THICKNESS_STEP, THICKNESS_STEP));
                    o.is_selection_change_needed = true;
                }
            }
            HeldKey::None => (),
        }
        (world, history)
    }
}

impl Clone for Brush {
    fn clone(&self) -> Self {
        Self {
            mover: self.mover,
            prediction_receiver: None, // JoinHandle cannot be cloned
        }
    }
}
impl Manipulate for Brush {
    fn new() -> Self {
        Self {
            mover: Mover::new(),
            prediction_receiver: None,
        }
    }

    fn on_filechange(&mut self, mut world: World, mut history: History) -> (World, History) {
        use_currentimageshape_for_annos(&mut world);

        let brush_data = get_specific_mut(&mut world);
        if let Some(brush_data) = brush_data {
            for (_, (anno, _)) in brush_data.anno_iter_mut() {
                anno.deselect_all();
            }
            let ild = get_instance_label_display(&world);
            world = instance_label_display_sort::<_, DataAccessors, InstanceAnnoAccessors>(
                world, ild, ACTOR_NAME,
            );
        }
        (world, history) =
            check_autopaste::<_, DataAccessors, InstanceAnnoAccessors>(world, history, ACTOR_NAME);
        set_visible(&mut world);
        (world, history)
    }
    fn on_activate(&mut self, mut world: World) -> World {
        if let Some(data) = trace_ok_err(get_data_mut(&mut world)) {
            data.menu_active = true;
        }
        set_visible(&mut world);
        world
    }
    fn on_deactivate(&mut self, mut world: World) -> World {
        if let Some(data) = trace_ok_err(get_data_mut(&mut world)) {
            data.menu_active = false;
        }
        world.request_redraw_annotations(BRUSH_NAME, Visibility::None);
        world
    }
    fn on_always_active_zoom(&mut self, mut world: World, history: History) -> (World, History) {
        let visible = get_options(&world).map(|o| o.core.visible) == Some(true);
        let vis = vis_from_lfoption(get_label_info(&world), visible);
        world.request_redraw_annotations(BRUSH_NAME, vis);
        (world, history)
    }
    fn events_tf(
        &mut self,
        mut world: World,
        mut history: History,
        events: &Events,
    ) -> (World, History) {
        world = check_trigger_redraw::<DataAccessors>(world, BRUSH_NAME);
        (world, history) =
            check_trigger_history_update::<DataAccessors>(world, history, BRUSH_NAME);
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
        predictive_labeling::<DataAccessors>(
            &mut world,
            &mut history,
            ACTOR_NAME,
            &mut self.prediction_receiver,
        );
        world = check_recolorboxes::<DataAccessors>(world, BRUSH_NAME);
        world = check_selected_intensity_thickness(world);
        world = check_export(world);
        make_tool_transform!(
            self,
            world,
            history,
            events,
            [
                (pressed, KeyCode::MouseLeft, mouse_pressed),
                (pressed, KeyCode::MouseRight, mouse_pressed),
                (held, KeyCode::MouseLeft, mouse_held),
                (held, KeyCode::MouseRight, mouse_held),
                (released, KeyCode::MouseLeft, mouse_released),
                (released, KeyCode::Back, key_released),
                (released, KeyCode::Delete, key_released),
                (released, KeyCode::A, key_released),
                (released, KeyCode::C, key_released),
                (released, KeyCode::D, key_released),
                (released, KeyCode::E, key_released),
                (released, KeyCode::H, key_released),
                (held, KeyCode::I, key_held),
                (released, KeyCode::L, key_released),
                (held, KeyCode::T, key_held),
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

#[cfg(test)]
use {
    crate::{
        tracing_setup::init_tracing_for_tests,
        types::{ThumbIms, ViewImage},
    },
    image::DynamicImage,
};

#[cfg(test)]
pub fn test_data() -> (Option<PtF>, World, History) {
    use std::path::Path;

    use crate::ToolsDataMap;
    let im_test = DynamicImage::ImageRgb8(ViewImage::new(64, 64));
    let mut world = World::from_real_im(
        im_test,
        ThumbIms::default(),
        ToolsDataMap::new(),
        None,
        Some("superimage.png".to_string()),
        Path::new("superimage.png"),
        Some(0),
    );
    world.data.meta_data.flags.is_loading_screen_active = Some(false);
    get_specific_mut(&mut world)
        .unwrap()
        .label_info
        .push("label".to_string(), None, None)
        .unwrap();
    let history = History::default();
    let mouse_pos = Some((32.0, 32.0).into());
    (mouse_pos, world, history)
}

#[test]
fn test_mouse_released() {
    init_tracing_for_tests();
    let (mp, mut world, history) = test_data();
    let options = get_options_mut(&mut world).unwrap();
    options.thickness = 1.0;
    let mut events = Events::default();
    events.mouse_pos_on_orig = mp;
    let (world, history) = mouse_released(&events, world, history);
    let annos = get_annos(&world).unwrap();
    assert_eq!(annos.len(), 1);
    assert_eq!(annos.elts()[0].bb.x, 32);
    assert_eq!(annos.elts()[0].bb.y, 32);
    events.mouse_pos_on_orig = Some((40, 40).into());
    let world = mouse_pressed_left(&events, world);
    let (world, history) = mouse_released(&events, world, history);
    let annos = get_annos(&world).unwrap();
    assert_eq!(annos.len(), 2);
    assert_eq!(annos.elts()[0].bb.x, 32);
    assert_eq!(annos.elts()[0].bb.y, 32);
    assert_eq!(annos.elts()[1].bb.x, 40);
    assert_eq!(annos.elts()[1].bb.y, 40);
    events.mouse_pos_on_orig = Some((10, 10).into());
    let (world, _) = mouse_released(&events, world, history);
    let annos = get_annos(&world).unwrap();
    assert_eq!(annos.len(), 3);
    assert_eq!(annos.elts()[0].bb.x, 32);
    assert_eq!(annos.elts()[0].bb.y, 32);
    assert_eq!(annos.elts()[1].bb.x, 40);
    assert_eq!(annos.elts()[1].bb.y, 40);
    assert_eq!(annos.elts()[2].bb.x, 10);
    assert_eq!(annos.elts()[2].bb.y, 10);
}
