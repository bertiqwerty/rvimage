use std::{cmp::Ordering, io::Cursor, mem, path::Path, thread};

use image::{codecs::png, EncodableLayout, ImageBuffer, ImageEncoder, Luma};
use imageproc::drawing::{draw_filled_circle_mut, BresenhamLineIter};
use tracing::{error, info};

use crate::{
    annotations_accessor, annotations_accessor_mut,
    domain::{BrushLine, PtF},
    events::{Events, KeyCode},
    file_util::osstr_to_str,
    history::{History, Record},
    make_tool_transform,
    result::{trace_ok, RvResult},
    tools::core::{check_recolorboxes, check_trigger_history_update, check_trigger_redraw},
    tools_data::{self, annotations::InstanceAnnotations, brush_data, ToolsData},
    tools_data::{annotations::BrushAnnotations, brush_mut},
    world::World,
    Line,
};

use super::{
    core::{deselect_all, label_change_key, map_released_key, on_selection_keys, ReleasedKey},
    Manipulate, BRUSH_NAME,
};

pub const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";
const MISSING_DATA_MSG: &str = "brush data not available";

annotations_accessor_mut!(ACTOR_NAME, brush_mut, MISSING_ANNO_MSG, BrushAnnotations);
annotations_accessor!(ACTOR_NAME, brush, MISSING_ANNO_MSG, BrushAnnotations);

const MAX_SELECT_DIST: f64 = 50.0;

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

fn find_closest_brushline(annos: &InstanceAnnotations<BrushLine>, p: PtF) -> Option<(usize, f64)> {
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

fn check_selected_intensity_thickness(mut world: World) -> World {
    let options = get_options(&world);
    let annos = get_annos_mut(&mut world);
    if let (Some(annos), Some(options)) = (annos, options) {
        if options.is_selection_change_needed {
            for brushline in annos.selected_elts_iter_mut() {
                brushline.intensity = options.intensity;
                brushline.thickness = options.thickness;
            }
        }
    }
    let options_mut = get_options_mut(&mut world);
    if let Some(options_mut) = options_mut {
        options_mut.is_selection_change_needed = false;
    }
    world
}

fn check_export(mut world: World) -> World {
    let options = get_options(&world);
    let specifics = get_specific(&world);

    if options.map(|o| o.core_options.is_export_triggered) == Some(true) {
        if let Some(data) = specifics {
            let annotations_map = data.annotations_map.clone();
            let ssh_cfg = world.data.meta_data.ssh_cfg.clone();
            let label_info = data.label_info.clone();
            let export_folder = data.export_folder.clone();
            let f_export = move || {
                for (filename, annos) in &annotations_map {
                    for label in label_info.labels() {
                        let (annos, shape) = annos;
                        if !annos.elts().is_empty() {
                            let mut im = ImageBuffer::<Luma<u8>, Vec<u8>>::new(shape.w, shape.h);
                            for brushline in annos
                                .elts()
                                .iter()
                                .zip(annos.cat_idxs().iter())
                                .filter(|(_, cat_idx)| &label_info.labels()[**cat_idx] == label)
                                .map(|(bl, _)| bl)
                            {
                                let p1_iter = brushline.line.points_iter();
                                let mut p2_iter = brushline.line.points_iter();
                                p2_iter.next();
                                for (p1, p2) in p1_iter.zip(p2_iter) {
                                    let lineseg_iter = BresenhamLineIter::new(
                                        (p1.x as f32, p1.y as f32),
                                        (p2.x as f32, p2.y as f32),
                                    );
                                    for center in lineseg_iter {
                                        draw_filled_circle_mut(
                                            &mut im,
                                            center,
                                            brushline.thickness as i32 / 2,
                                            Luma([(brushline.intensity * 255.0) as u8]),
                                        );
                                    }
                                }
                            }
                            let filepath = Path::new(&filename);
                            let outfilename = format!(
                                "{}_{label}.png",
                                osstr_to_str(filepath.file_stem()).unwrap_or_else(|_| panic!(
                                    "a filepath needs a stem, what's wrong with {filepath:?}"
                                ))
                            );
                            let outpath = export_folder.path.join(outfilename);
                            let mut buffer = Cursor::new(vec![]);
                            let encoder = png::PngEncoder::new_with_quality(
                                &mut buffer,
                                png::CompressionType::Best,
                                png::FilterType::NoFilter,
                            );
                            if let Err(e) = encoder.write_image(
                                im.as_bytes(),
                                shape.w,
                                shape.h,
                                image::ColorType::L8,
                            ) {
                                error!("could not decode png due to {e:?}");
                            } else if let Err(e) = export_folder.conn.write_bytes(
                                buffer.get_ref(),
                                &outpath,
                                ssh_cfg.as_ref(),
                            ) {
                                error!("export failed due to {e:?}");
                            } else {
                                info!("exported label file to '{outpath:?}'");
                            }
                        }
                    }
                }
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
            world = deselect_all(world, BRUSH_NAME, get_annos_mut);
        }
        if !events.held_ctrl() {
            let options = get_options(&world);
            let cat_idx = get_specific(&world).map(|d| d.label_info.cat_idx_current);
            if let (Some(mp), Some(annos), Some(options), Some(cat_idx)) = (
                events.mouse_pos_on_orig,
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
            if let (Some(mp), Some(annos)) = (events.mouse_pos_on_orig, get_annos_mut(&mut world)) {
                if erase != Some(true) {
                    if let Some(line) = annos.last_line_mut() {
                        let last_point = line.last_point();
                        let dist = if let Some(last_point) = last_point {
                            last_point.dist_square(&mp)
                        } else {
                            100.0
                        };
                        if dist >= 1.0 {
                            line.push(mp);
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
            if let (Some(mp), Some(annos)) = (events.mouse_pos_on_orig, get_annos_mut(&mut world)) {
                let to_be_selected_line_idx = find_closest_brushline(annos, mp);
                if let Some((idx, dist)) = to_be_selected_line_idx {
                    let thickness = annos.elts()[idx].thickness;
                    if dist < MAX_SELECT_DIST + thickness {
                        if annos.selected_mask()[idx] {
                            annos.deselect(idx);
                        } else {
                            annos.select(idx);
                        }
                        world.request_redraw_annotations(BRUSH_NAME, true)
                    } else {
                        world = deselect_all(world, BRUSH_NAME, get_annos_mut);
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
        mut history: History,
        events: &Events,
    ) -> (World, History) {
        world = check_trigger_redraw(world, BRUSH_NAME, |d| {
            brush_mut(d).map(|d| &mut d.options.core_options)
        });
        (world, history) = check_trigger_history_update(world, history, BRUSH_NAME, |d| {
            brush_mut(d).map(|d| &mut d.options.core_options)
        });
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
