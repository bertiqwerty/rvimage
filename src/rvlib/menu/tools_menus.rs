use std::{collections::HashMap, fmt::Display, mem, path::PathBuf, str::FromStr};

use egui::Ui;
use tracing::{info, warn};

use crate::{
    cfg::{ExportPath, ExportPathConnection},
    domain::{Annotate, TPtF, TPtI},
    file_util::path_to_str,
    result::{to_rv, RvResult},
    tools_data::{
        annotations::{InstanceAnnotations, SplitMode},
        attributes_data::AttrVal,
        bbox_data::{BboxSpecificData, ImportMode},
        brush_data::{MAX_INTENSITY, MAX_THICKNESS, MIN_INTENSITY, MIN_THICKNESS},
        AttributesToolData, BrushToolData, CoreOptions, LabelInfo, ToolSpecifics, ToolsData,
        OUTLINE_THICKNESS_CONVERSION,
    },
    ShapeI,
};

use super::ui_util::{slider, text_edit_singleline};

fn new_label_text(
    ui: &mut Ui,
    new_label: &mut String,
    are_tools_active: &mut bool,
) -> Option<String> {
    let label_field = text_edit_singleline(ui, new_label, are_tools_active);
    if label_field.lost_focus() {
        Some(new_label.clone())
    } else {
        None
    }
}
#[derive(Default)]
pub struct LabelMenuResult {
    pub label_change: bool,
    pub show_only_change: bool,
}

pub fn label_menu<'a, T>(
    ui: &mut Ui,
    label_info: &mut LabelInfo,
    annotations_map: &mut HashMap<String, (InstanceAnnotations<T>, ShapeI)>,
    are_tools_active: &mut bool,
) -> LabelMenuResult
where
    T: Annotate + 'a,
{
    let mut new_idx = label_info.cat_idx_current;
    let mut label_change = false;
    let mut show_only_change = false;
    let new_label = new_label_text(ui, &mut label_info.new_label, are_tools_active);
    let default_label = label_info.find_default();
    if let (Some(default_label), Some(new_label)) = (default_label, new_label.as_ref()) {
        info!("replaced default '{default_label}' label by '{new_label}'");
        *default_label = new_label.clone();
        label_change = true;
    } else if let Some(new_label) = new_label {
        if let Err(e) = label_info.push(new_label, None, None) {
            warn!("{e:?}");
            return LabelMenuResult::default();
        }
        label_change = true;
        new_idx = label_info.len() - 1;
    }
    let mut to_be_removed = None;
    let mut show_only_current = label_info.show_only_current;
    for (label_idx, label) in label_info.labels().iter().enumerate() {
        let checked = label_idx == label_info.cat_idx_current;
        ui.horizontal_top(|ui| {
            if ui.button("x").clicked() {
                to_be_removed = Some(label_idx);
            }
            let label = if show_only_current && checked {
                egui::RichText::new(label).monospace().strong().italics()
            } else {
                egui::RichText::new(label).monospace()
            };
            if ui.selectable_label(checked, label).clicked() {
                if checked {
                    show_only_current = !label_info.show_only_current;
                    show_only_change = true;
                }
                new_idx = label_idx;
            }
            let rgb = label_info.colors()[label_idx];
            ui.label(
                egui::RichText::new("■")
                    .heading()
                    .strong()
                    .color(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])),
            );
        });
    }
    label_info.show_only_current = show_only_current;
    if new_idx != label_info.cat_idx_current {
        for (annos, _) in annotations_map.values_mut() {
            annos.label_selected(new_idx);
        }
        label_change = true;
        label_info.cat_idx_current = new_idx;
    }
    if let Some(tbr) = to_be_removed {
        label_change = true;
        label_info.remove_catidx(tbr, annotations_map)
    }
    if label_change {
        label_info.show_only_current = false;
    }
    LabelMenuResult {
        label_change,
        show_only_change,
    }
}

fn hide_menu(ui: &mut Ui, mut core_options: CoreOptions) -> CoreOptions {
    let mut hide = !core_options.visible;
    if ui.checkbox(&mut hide, "hide").changed() {
        core_options.is_redraw_annos_triggered = true;
        core_options.visible = !hide;
    }
    core_options
}

fn export_file_menu(
    ui: &mut Ui,
    label: &str,
    export_path: &mut ExportPath,
    are_tools_active: &mut bool,
    is_export_triggered: &mut bool,
    is_import_triggered: Option<&mut bool>,
    import_mode: Option<&mut ImportMode>,
) -> RvResult<()> {
    let mut file_txt = path_to_str(&export_path.path)?.to_string();
    ui.horizontal(|ui| {
        ui.label(label);
        ui.radio_value(&mut export_path.conn, ExportPathConnection::Local, "local");
        ui.radio_value(&mut export_path.conn, ExportPathConnection::Ssh, "ssh");
    });
    text_edit_singleline(ui, &mut file_txt, are_tools_active);
    if path_to_str(&export_path.path)? != file_txt {
        export_path.path = PathBuf::from_str(&file_txt).map_err(to_rv)?;
    }
    ui.horizontal(|ui| {
        if ui.button("export").clicked() {
            tracing::info!("export triggered");
            *is_export_triggered = true;
        }
        if let (Some(is_import_triggered), Some(import_mode)) = (is_import_triggered, import_mode) {
            if ui.button("import").clicked() {
                tracing::info!("import triggered");
                *is_import_triggered = true;
            }
            let mut checked = *import_mode == ImportMode::Merge;
            ui.checkbox(&mut checked, "merge import");
            if checked {
                *import_mode = ImportMode::Merge;
            } else {
                *import_mode = ImportMode::Replace;
            }
        }
    });
    Ok(())
}

fn toggle_erase(ui: &mut Ui, mut options: CoreOptions) -> CoreOptions {
    if ui.checkbox(&mut options.erase, "erase").clicked() {
        if options.erase {
            info!("start erasing");
        } else {
            info!("stop erasing");
        }
    }
    options
}
pub fn bbox_menu(
    ui: &mut Ui,
    mut window_open: bool,
    mut data: BboxSpecificData,
    are_tools_active: &mut bool,
) -> RvResult<ToolsData> {
    let LabelMenuResult {
        label_change,
        show_only_change,
    } = label_menu(
        ui,
        &mut data.label_info,
        &mut data.annotations_map,
        are_tools_active,
    );
    if label_change {
        data.options.core_options = data.options.core_options.trigger_redraw_and_hist();
    }
    if show_only_change {
        data.options.core_options.is_redraw_annos_triggered = true;
    }
    ui.separator();

    data.options.core_options = toggle_erase(ui, data.options.core_options);
    data.options.core_options = hide_menu(ui, data.options.core_options);

    ui.checkbox(&mut data.options.auto_paste, "auto paste");

    let mut export_file_menu_result = Ok(());
    egui::CollapsingHeader::new("advanced").show(ui, |ui| {
        let mut transparency: f32 = data.options.fill_alpha as f32 / 255.0 * 100.0;
        ui.label("transparency");
        if slider(ui, are_tools_active, &mut transparency, 0.0..=100.0, "fill").changed() {
            data.options.core_options.is_redraw_annos_triggered = true;
        }
        data.options.fill_alpha = (transparency / 100.0 * 255.0).round() as u8;
        let mut transparency = data.options.outline_alpha as f32 / 255.0 * 100.0;
        if slider(
            ui,
            are_tools_active,
            &mut transparency,
            0.0..=100.0,
            "outline",
        )
        .changed()
        {
            data.options.core_options.is_redraw_annos_triggered = true;
        }
        data.options.outline_alpha = (transparency / 100.0 * 255.0).round() as u8;
        let mut outline_thickness_f =
            data.options.outline_thickness as TPtF / OUTLINE_THICKNESS_CONVERSION;
        ui.separator();
        if slider(
            ui,
            are_tools_active,
            &mut outline_thickness_f,
            0.0..=10.0,
            "outline thickness",
        )
        .changed()
        {
            data.options.core_options.is_redraw_annos_triggered = true;
        }
        data.options.outline_thickness =
            (outline_thickness_f * OUTLINE_THICKNESS_CONVERSION).round() as u16;

        ui.separator();
        if slider(
            ui,
            are_tools_active,
            &mut data.options.drawing_distance,
            1..=50,
            "drawing distance parameter",
        )
        .changed()
        {
            data.options.core_options.is_redraw_annos_triggered = true;
        }
        ui.horizontal(|ui| {
            ui.separator();
            ui.label("split mode");
            ui.radio_value(&mut data.options.split_mode, SplitMode::None, "none");
            ui.radio_value(
                &mut data.options.split_mode,
                SplitMode::Horizontal,
                "horizontal",
            );
            ui.radio_value(
                &mut data.options.split_mode,
                SplitMode::Vertical,
                "vertical",
            );
        });

        ui.separator();

        export_file_menu_result = export_file_menu(
            ui,
            "coco file",
            &mut data.coco_file,
            are_tools_active,
            &mut data.options.core_options.is_export_triggered,
            Some(&mut data.options.is_import_triggered),
            Some(&mut data.options.import_mode),
        );

        ui.separator();
        if ui.button("new random colors").clicked() {
            data.options.core_options.is_colorchange_triggered = true;
        }
        if ui.button("clear out of folder annotations").clicked() {
            data.options.is_anno_rm_triggered = true;
        }
    });
    export_file_menu_result?;
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("close").clicked() {
            window_open = false;
        }
    });
    Ok(ToolsData {
        specifics: ToolSpecifics::Bbox(data),
        menu_active: window_open,
    })
}

pub fn brush_menu(
    ui: &mut Ui,
    mut window_open: bool,
    mut data: BrushToolData,
    are_tools_active: &mut bool,
) -> RvResult<ToolsData> {
    let LabelMenuResult {
        label_change,
        show_only_change,
    } = label_menu(
        ui,
        &mut data.label_info,
        &mut data.annotations_map,
        are_tools_active,
    );
    if label_change {
        data.options.core_options = data.options.core_options.trigger_redraw_and_hist();
    }
    if show_only_change {
        data.options.core_options.is_redraw_annos_triggered = true;
    }

    data.options.core_options = toggle_erase(ui, data.options.core_options);
    data.options.core_options = hide_menu(ui, data.options.core_options);
    if slider(
        ui,
        are_tools_active,
        &mut data.options.thickness,
        MIN_THICKNESS..=MAX_THICKNESS,
        "thickness",
    )
    .changed()
    {
        data.options.is_selection_change_needed = true;
    }
    if slider(
        ui,
        are_tools_active,
        &mut data.options.intensity,
        MIN_INTENSITY..=MAX_INTENSITY,
        "intensity",
    )
    .changed()
    {
        data.options.is_selection_change_needed = true;
    }
    if ui.button("new random colors").clicked() {
        data.options.core_options.is_colorchange_triggered = true;
    }
    export_file_menu(
        ui,
        "png export folder",
        &mut data.export_folder,
        are_tools_active,
        &mut data.options.core_options.is_export_triggered,
        None,
        None,
    )?;
    if ui.button("close").clicked() {
        window_open = false;
    }
    Ok(ToolsData {
        specifics: ToolSpecifics::Brush(data),
        menu_active: window_open,
    })
}

fn removable_rows(
    ui: &mut Ui,
    n_rows: usize,
    mut make_row: impl FnMut(&mut Ui, usize),
) -> Option<usize> {
    let mut to_be_removed = None;
    for idx in 0..n_rows {
        if ui.button("x").clicked() {
            to_be_removed = Some(idx);
        }
        make_row(ui, idx)
    }
    to_be_removed
}

fn process_number<T>(
    x: &mut T,
    ui: &mut Ui,
    are_tools_active: &mut bool,
    label: &str,
    buffer: &mut String,
) -> bool
where
    T: Display + FromStr,
{
    let new_val = text_edit_singleline(ui, buffer, are_tools_active).on_hover_text(label);
    if new_val.changed() {
        match buffer.parse::<T>() {
            Ok(val) => {
                *x = val;
            }
            Err(_) => {
                warn!("could not parse {buffer} as number");
            }
        }
    }
    new_val.lost_focus()
}
pub fn attributes_menu(
    ui: &mut Ui,
    mut window_open: bool,
    mut data: AttributesToolData,
    are_tools_active: &mut bool,
) -> RvResult<ToolsData> {
    const FLOAT_LABEL: &str = "Float";
    const INT_LABEL: &str = "Int";
    const TEXT_LABEL: &str = "Text";
    const BOOL_LABEL: &str = "Bool";
    text_edit_singleline(ui, &mut data.new_attr, are_tools_active);
    ui.horizontal(|ui| {
        egui::ComboBox::from_label("")
            .selected_text(format!(
                "{:?}",
                match data.new_attr_type {
                    AttrVal::Float(_) => FLOAT_LABEL,
                    AttrVal::Int(_) => INT_LABEL,
                    AttrVal::Str(_) => TEXT_LABEL,
                    AttrVal::Bool(_) => BOOL_LABEL,
                }
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut data.new_attr_type,
                    AttrVal::Float(TPtF::default()),
                    FLOAT_LABEL,
                );
                ui.selectable_value(
                    &mut data.new_attr_type,
                    AttrVal::Int(TPtI::default()),
                    INT_LABEL,
                );
                ui.selectable_value(
                    &mut data.new_attr_type,
                    AttrVal::Str(String::new()),
                    TEXT_LABEL,
                );
                ui.selectable_value(&mut data.new_attr_type, AttrVal::Bool(false), BOOL_LABEL);
            });
        if ui.button("Add").clicked() {
            if data.attr_names().contains(&data.new_attr) {
                warn!("attribute {:?} already exists", data.new_attr);
            }
            data.push(data.new_attr.clone(), data.new_attr_type.clone());
            data.options.populate_new_attr = true;
            data.options.update_current_attr_map = true;
        }
    });
    egui::Grid::new("attributes_grid")
        .num_columns(4)
        .show(ui, |ui| {
            ui.end_row();
            let n_rows = data.attr_names().len();
            let to_be_removed = removable_rows(ui, n_rows, |ui, idx| {
                let attr_name = data.attr_names()[idx].clone();
                let mut new_attr_buffer = mem::take(data.attr_buffer_mut(idx));
                ui.label(&attr_name);
                let attr_map = &mut data.current_attr_map;
                if let Some(attr_map) = attr_map {
                    match attr_map.get_mut(&attr_name) {
                        Some(AttrVal::Bool(b)) => {
                            if ui.checkbox(b, "").changed() {
                                data.options.update_current_attr_map = true;
                            }
                        }
                        Some(AttrVal::Float(x)) => {
                            let lost_focus = process_number(
                                x,
                                ui,
                                are_tools_active,
                                FLOAT_LABEL,
                                &mut new_attr_buffer,
                            );
                            if lost_focus || ui.button("OK").clicked() {
                                data.options.update_current_attr_map = true;
                            }
                        }
                        Some(AttrVal::Int(x)) => {
                            let lost_focus = process_number(
                                x,
                                ui,
                                are_tools_active,
                                INT_LABEL,
                                &mut new_attr_buffer,
                            );
                            if lost_focus || ui.button("OK").clicked() {
                                data.options.update_current_attr_map = true;
                            }
                        }
                        Some(AttrVal::Str(s)) => {
                            let lost_focus = text_edit_singleline(ui, s, are_tools_active)
                                .on_hover_text(TEXT_LABEL)
                                .lost_focus();
                            if lost_focus || ui.button("OK").clicked() {
                                data.options.update_current_attr_map = true;
                            }
                        }
                        None => {
                            warn!("attr_map does not contain {attr_name}");
                        }
                    }
                }
                *data.attr_buffer_mut(idx) = new_attr_buffer;
                ui.end_row();
            });
            if let Some(tbr) = to_be_removed {
                data.remove_attr(tbr);
            }
        });

    export_file_menu(
        ui,
        "export attributes as json",
        &mut data.export_path,
        are_tools_active,
        &mut data.options.is_export_triggered,
        None,
        None,
    )?;

    if ui.button("Close").clicked() {
        window_open = false;
    }
    Ok(ToolsData {
        specifics: ToolSpecifics::Attributes(data),
        menu_active: window_open,
    })
}
