use std::{cmp::Ordering, mem, thread};

use brush_data::BrushToolData;

use crate::{
    annotations_accessor, annotations_accessor_mut,
    cfg::ExportPath,
    domain::{BrushLine, Canvas, PtF, TPtF},
    events::{Events, KeyCode},
    file_util::MetaData,
    history::{History, Record},
    make_tool_transform,
    result::trace_ok,
    tools::{
        core::{check_recolorboxes, check_trigger_history_update, check_trigger_redraw},
        instance_anno_shared::check_cocoimport,
    },
    tools_data::{
        self,
        annotations::BrushAnnotations,
        brush_data::{self, MAX_INTENSITY, MAX_THICKNESS, MIN_INTENSITY, MIN_THICKNESS},
        brush_mut, vis_from_lfoption, InstanceAnnotate, LabelInfo, Rot90ToolData,
    },
    tools_data_accessors, tools_data_accessors_objects,
    util::Visibility,
    world::World,
    Annotation, BrushAnnotation, Line, ShapeI,
};

use super::{
    core::{
        check_erase_mode, deselect_all, label_change_key, map_held_key, map_released_key,
        on_selection_keys, HeldKey, ReleasedKey,
    },
    instance_anno_shared::get_rot90_data,
    Manipulate, BRUSH_NAME,
};

pub const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";
const MISSING_DATA_MSG: &str = "brush data not available";
annotations_accessor_mut!(ACTOR_NAME, brush_mut, MISSING_ANNO_MSG, BrushAnnotations);
annotations_accessor!(ACTOR_NAME, brush, MISSING_ANNO_MSG, BrushAnnotations);
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

fn import_coco_if_triggered(
    meta_data: &MetaData,
    coco_file: Option<&ExportPath>,
    rot90_data: Option<&Rot90ToolData>,
) -> Option<BrushToolData> {
    if let Some(coco_file) = coco_file {
        match tools_data::coco_io::read_coco(meta_data, coco_file, rot90_data) {
            Ok((_, brush_data)) => Some(brush_data),
            Err(e) => {
                tracing::error!("could not import coco due to {e:?}");
                None
            }
        }
    } else {
        None
    }
}
fn max_select_dist(shape: ShapeI) -> TPtF {
    (TPtF::from(shape.w.pow(2) + shape.h.pow(2)).sqrt() / 100.0).max(50.0)
}

fn draw_erase_circle(mut world: World, mp: PtF) -> World {
    let show_only_current = get_specific(&world).map(|d| d.label_info.show_only_current);
    let options = get_options(&world);
    let idx_current = get_specific(&world).map(|d| d.label_info.cat_idx_current);
    let annos = get_annos_mut(&mut world);
    if let (Some(options), Some(annos)) = (options, annos) {
        let to_be_removed_line_idx = find_closest_canvas(annos, mp, |idx| {
            annos.is_of_current_label(idx, idx_current, show_only_current)
        });
        if let Some((idx, _)) = to_be_removed_line_idx {
            let canvas = annos.edit(idx);
            canvas.draw_circle(mp, options.thickness, 0);
        }
        set_visible(&mut world);
    }
    world
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
    let options = get_options(&world);
    let annos = get_annos_mut(&mut world);
    let mut any_selected = false;
    if let (Some(annos), Some(options)) = (annos, options) {
        if options.is_selection_change_needed {
            for brushline in annos.selected_elts_iter_mut() {
                brushline.intensity = options.intensity;
                any_selected = true;
            }
        }
    }
    let options_mut = get_options_mut(&mut world);
    if let Some(options_mut) = options_mut {
        options_mut.is_selection_change_needed = false;
        if any_selected {
            options_mut.core_options.is_redraw_annos_triggered = true;
        }
    }
    world
}

fn check_export(mut world: World) -> World {
    let options = get_options(&world);
    let specifics = get_specific(&world);

    if options.map(|o| o.core_options.is_export_triggered) == Some(true) {
        let rot90_data = get_rot90_data(&world).cloned();
        if let Some(data) = specifics {
            let meta_data = world.data.meta_data.clone();
            let data = data.clone();
            let f_export =
                move || match tools_data::write_coco(&meta_data, data, rot90_data.as_ref()) {
                    Ok(p) => tracing::info!("export to {p:?} successful"),
                    Err(e) => tracing::error!("export failed due to {e:?}"),
                };
            thread::spawn(f_export);
        }
        if let Some(options_mut) = get_options_mut(&mut world) {
            options_mut.core_options.is_export_triggered = false;
        }
    }
    world
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
        if !(events.held_alt() || events.held_ctrl() || events.held_shift()) {
            world = deselect_all(world, BRUSH_NAME, get_annos_mut, get_label_info);
        }
        if !events.held_ctrl() {
            let options = get_options(&world);
            let idx_current = get_specific(&world).map(|d| d.label_info.cat_idx_current);
            if let (Some(mp), Some(options)) = (events.mouse_pos_on_orig, options) {
                let erase = options.core_options.erase;
                if !erase {
                    if let (Some(d), Some(cat_idx)) = (get_specific_mut(&mut world), idx_current) {
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
            }
            set_visible(&mut world);
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
            let options = get_options(&world);
            if let (Some(mp), Some(options)) = (events.mouse_pos_on_orig, options) {
                if options.core_options.erase {
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
                        world.request_redraw_tmp_anno(Annotation::Brush(BrushAnnotation {
                            canvas: Canvas::new(&line, world.shape_orig()).unwrap(),
                            tmp_line: Some(line.clone()),
                            color,
                            label: None,
                            is_selected: None,
                            fill_alpha: options.fill_alpha,
                        }));
                    }
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
                        world = deselect_all(world, BRUSH_NAME, get_annos_mut, get_label_info);
                    }
                }
            }
            set_visible(&mut world);
        } else if !(events.held_alt() || events.held_shift()) {
            // neither shift nor alt nor ctrl were held => a brushline has been finished
            // or a brush line has been deleted.
            let erase = get_options(&world).map(|o| o.core_options.erase);
            let cat_idx = get_specific(&world).map(|o| o.label_info.cat_idx_current);
            if erase != Some(true) {
                let shape_orig = world.shape_orig();
                let line = get_specific(&world).and_then(|d| d.tmp_line.clone());
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

                let maybe_annos = get_annos_mut(&mut world);
                if let (Some(annos), Some(line), Some(cat_idx)) = (maybe_annos, line, cat_idx) {
                    let canvas = Canvas::new(&line, shape_orig);
                    if let Ok(canvas) = canvas {
                        annos.add_elt(canvas, cat_idx);
                    }
                }
                if let Some(d) = get_specific_mut(&mut world) {
                    d.tmp_line = None;
                }
                set_visible(&mut world);
            } else if let Some(mp) = events.mouse_pos_on_orig {
                world = draw_erase_circle(world, mp);
            }
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
        let held_key = map_held_key(events);
        const INTENSITY_STEP: f64 = MAX_INTENSITY / 20.0;
        const THICKNESS_STEP: f64 = MAX_THICKNESS / 20.0;
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
            _ => (),
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
        (world, history) = on_selection_keys(
            world,
            history,
            released_key,
            events.held_ctrl(),
            BRUSH_NAME,
            get_annos_mut,
            |world| get_specific_mut(world).map(|d| &mut d.clipboard),
            |world| get_options(world).map(|o| o.core_options),
            get_label_info,
        );
        let mut trigger_redraw = false;
        if let Some(label_info) = get_specific_mut(&mut world).map(|s| &mut s.label_info) {
            (*label_info, trigger_redraw) = label_change_key(released_key, mem::take(label_info));
        }
        if trigger_redraw {
            let visible = get_options(&world).map(|o| o.core_options.visible) == Some(true);
            let vis = vis_from_lfoption(get_label_info(&world), visible);
            world.request_redraw_annotations(BRUSH_NAME, vis);
        }
        match released_key {
            ReleasedKey::H if events.held_ctrl() => {
                // Hide all boxes (selected or not)
                if let Some(options_mut) = get_options_mut(&mut world) {
                    options_mut.core_options.visible = !options_mut.core_options.visible;
                }
                let vis = get_visible(&world);
                world.request_redraw_annotations(BRUSH_NAME, vis);
            }
            _ => (),
        }
        world = check_erase_mode(
            released_key,
            |w| get_options_mut(w).map(|o| &mut o.core_options),
            set_visible,
            world,
        );
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
        set_visible(&mut world);
        (world, history)
    }
    fn on_activate(&mut self, mut world: World) -> World {
        if let Some(data) = trace_ok(get_data_mut(&mut world)) {
            data.menu_active = true;
        }
        set_visible(&mut world);
        world
    }
    fn on_deactivate(&mut self, mut world: World) -> World {
        if let Some(td) = world.data.tools_data_map.get_mut(BRUSH_NAME) {
            td.menu_active = false;
        }
        world.request_redraw_annotations(BRUSH_NAME, Visibility::None);
        world
    }
    fn on_always_active_zoom(&mut self, mut world: World, history: History) -> (World, History) {
        let visible = get_options(&world).map(|o| o.core_options.visible) == Some(true);
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
        world = check_trigger_redraw(world, BRUSH_NAME, get_label_info, |d| {
            brush_mut(d).map(|d| &mut d.options.core_options)
        });
        (world, history) = check_trigger_history_update(world, history, BRUSH_NAME, |d| {
            brush_mut(d).map(|d| &mut d.options.core_options)
        });
        let imported;
        (world, imported) = check_cocoimport(
            world,
            |w| get_options(w).map(|o| o.core_options),
            get_rot90_data,
            get_specific,
            get_specific_mut,
            import_coco_if_triggered,
        );
        if imported {
            set_visible(&mut world);
        }
        world = check_recolorboxes(
            world,
            BRUSH_NAME,
            |world| get_options_mut(world).map(|o| &mut o.core_options),
            |world| get_specific_mut(world).map(|d| &mut d.label_info),
        );
        world = check_selected_intensity_thickness(world);
        world = check_export(world);
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
                (held, KeyCode::I, key_held),
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
