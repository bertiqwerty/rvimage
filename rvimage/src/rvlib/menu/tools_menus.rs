use std::{
    fmt::{Debug, Display},
    mem,
    path::PathBuf,
    str::FromStr,
};

use crate::{
    cfg::{ExportPath, ExportPathConnection},
    file_util::path_to_str,
    menu::ui_util::process_number,
    result::trace_ok_err,
    tools::{get_visible_inactive_names, BBOX_NAME, BRUSH_NAME},
    tools_data::{
        annotations::SplitMode,
        attributes_data::AttrVal,
        bbox_data::BboxToolData,
        brush_data::{MAX_INTENSITY, MAX_THICKNESS, MIN_INTENSITY, MIN_THICKNESS},
        AnnotationsMap, AttributesToolData, BrushToolData, CoreOptions, ImportExportTrigger,
        InstanceAnnotate, LabelInfo, ToolSpecifics, ToolsData, VisibleInactiveToolsState,
        OUTLINE_THICKNESS_CONVERSION,
    },
};
use egui::Ui;
use rvimage_domain::TPtF;
use rvimage_domain::{to_rv, RvResult};
use tracing::{info, warn};

use super::ui_util::{slider, text_edit_singleline};

fn removable_rows(
    ui: &mut Ui,
    n_rows: usize,
    mut make_row: impl FnMut(&mut Ui, usize),
) -> Option<usize> {
    let mut to_be_removed = None;
    for idx in 0..n_rows {
        if ui
            .button("x")
            .on_hover_text("double click😈")
            .double_clicked()
        {
            to_be_removed = Some(idx);
        }
        make_row(ui, idx)
    }
    to_be_removed
}
enum LabelEditMode {
    Add,
    Rename,
}

fn new_label_text(
    ui: &mut Ui,
    new_label: &mut String,
    are_tools_active: &mut bool,
) -> Option<(String, LabelEditMode)> {
    text_edit_singleline(ui, new_label, are_tools_active);
    ui.horizontal(|ui| {
        if ui.button("add").clicked() {
            Some((new_label.clone(), LabelEditMode::Add))
        } else if ui.button("rename").clicked() {
            Some((new_label.clone(), LabelEditMode::Rename))
        } else {
            None
        }
    })
    .inner
}

fn show_inactive_tool_menu(
    ui: &mut Ui,
    tool_name: &'static str,
    visible: &mut VisibleInactiveToolsState,
) -> bool {
    ui.label("Show inactive tool");
    let mut changed = false;
    let inactives = get_visible_inactive_names(tool_name);
    for (name, show) in inactives.iter().zip(visible.iter_mut()) {
        changed |= ui.checkbox(show, *name).changed();
    }
    changed
}

#[derive(Default)]
pub struct LabelMenuResult {
    pub label_change: bool,
    pub show_only_change: bool,
}

pub fn label_menu<'a, T>(
    ui: &mut Ui,
    label_info: &mut LabelInfo,
    annotations_map: &mut AnnotationsMap<T>,
    are_tools_active: &mut bool,
) -> LabelMenuResult
where
    T: InstanceAnnotate + 'a,
{
    let mut new_idx = label_info.cat_idx_current;
    let mut label_change = false;
    let mut show_only_change = false;
    let new_label = new_label_text(ui, &mut label_info.new_label, are_tools_active);
    let default_label = label_info.find_default();
    if let (Some(default_label), Some((new_label, _))) = (default_label, new_label.as_ref()) {
        info!("replaced default '{default_label}' label by '{new_label}'");
        default_label.clone_from(new_label);
        label_change = true;
    } else if let Some((new_label, edit_mode)) = new_label {
        match edit_mode {
            LabelEditMode::Add => {
                if let Err(e) = label_info.push(new_label, None, None) {
                    warn!("{e:?}");
                    return LabelMenuResult::default();
                }
                label_change = true;
                new_idx = label_info.len() - 1;
            }
            LabelEditMode::Rename => {
                if let Err(e) = label_info.rename_label(label_info.cat_idx_current, new_label) {
                    warn!("{e:?}");
                    return LabelMenuResult::default();
                }
                label_change = true;
            }
        }
    }
    let mut show_only_current = label_info.show_only_current;
    let mut to_be_removed = None;
    let n_rows = label_info.labels().len();
    egui::Grid::new("label_grid").num_columns(3).show(ui, |ui| {
        to_be_removed = removable_rows(ui, n_rows, |ui, label_idx| {
            let label = &label_info.labels()[label_idx];
            let checked = label_idx == label_info.cat_idx_current;
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
            ui.end_row();
        });
    });
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
    import_export_trigger: &mut ImportExportTrigger,
    skip_import_mode: bool,
) -> RvResult<()> {
    let mut file_txt = path_to_str(&export_path.path)?.to_string();
    ui.horizontal(|ui| {
        ui.label(label);
        ui.radio_value(&mut export_path.conn, ExportPathConnection::Local, "local");
        ui.radio_value(&mut export_path.conn, ExportPathConnection::Ssh, "ssh");
    });
    text_edit_singleline(ui, &mut file_txt, are_tools_active)
        .on_hover_text(path_to_str(&export_path.path)?);

    if path_to_str(&export_path.path)? != file_txt {
        export_path.path = PathBuf::from_str(&file_txt).map_err(to_rv)?;
    }
    ui.horizontal(|ui| {
        if ui.button("export").clicked() {
            tracing::info!("clicked on export trigger");
            import_export_trigger.trigger_export();
        }
        if ui.button("import").clicked() {
            tracing::info!("clicked on import trigger");
            import_export_trigger.trigger_import();
        }
        if skip_import_mode {
            let mut checked = import_export_trigger.merge_mode();
            ui.checkbox(&mut checked, "merge import");
            if checked {
                import_export_trigger.use_merge_import();
            } else {
                import_export_trigger.use_replace_import();
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
fn transparency_slider(
    ui: &mut Ui,
    are_tools_active: &mut bool,
    alpha: &mut u8,
    name: &str,
) -> bool {
    let mut transparency: f32 = *alpha as f32 / 255.0 * 100.0;
    let is_redraw_triggered =
        slider(ui, are_tools_active, &mut transparency, 0.0..=100.0, name).changed();
    *alpha = (transparency / 100.0 * 255.0).round() as u8;
    is_redraw_triggered
}
pub fn bbox_menu(
    ui: &mut Ui,
    mut window_open: bool,
    mut data: BboxToolData,
    are_tools_active: &mut bool,
    mut visible_inactive_tools: VisibleInactiveToolsState,
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
        data.options.core = data.options.core.trigger_redraw_and_hist();
    }
    if show_only_change {
        data.options.core.is_redraw_annos_triggered = true;
    }
    ui.separator();

    data.options.core = toggle_erase(ui, data.options.core);
    data.options.core = hide_menu(ui, data.options.core);

    ui.checkbox(&mut data.options.core.auto_paste, "auto paste");

    let mut export_file_menu_result = Ok(());
    egui::CollapsingHeader::new("advanced").show(ui, |ui| {
        ui.checkbox(&mut data.options.core.track_changes, "track changes");
        if transparency_slider(
            ui,
            are_tools_active,
            &mut data.options.fill_alpha,
            "fill transparency",
        ) {
            data.options.core.is_redraw_annos_triggered = true;
        }
        if transparency_slider(
            ui,
            are_tools_active,
            &mut data.options.outline_alpha,
            "outline transparency",
        ) {
            data.options.core.is_redraw_annos_triggered = true;
        }
        let mut outline_thickness_f =
            data.options.outline_thickness as TPtF / OUTLINE_THICKNESS_CONVERSION;
        if slider(
            ui,
            are_tools_active,
            &mut outline_thickness_f,
            0.0..=10.0,
            "outline thickness",
        )
        .changed()
        {
            data.options.core.is_redraw_annos_triggered = true;
        }
        data.options.outline_thickness =
            (outline_thickness_f * OUTLINE_THICKNESS_CONVERSION).round() as u16;
        if slider(
            ui,
            are_tools_active,
            &mut data.options.drawing_distance,
            1..=50,
            "drawing distance parameter",
        )
        .changed()
        {
            data.options.core.is_redraw_annos_triggered = true;
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

        let skip_import_mode = false;
        export_file_menu_result = export_file_menu(
            ui,
            "coco file",
            &mut data.coco_file,
            are_tools_active,
            &mut data.options.core.import_export_trigger,
            skip_import_mode,
        );

        ui.separator();
        if ui.button("new random colors").clicked() {
            data.options.core.is_colorchange_triggered = true;
        }
    });
    export_file_menu_result?;
    ui.separator();
    if show_inactive_tool_menu(ui, BBOX_NAME, &mut visible_inactive_tools) {
        data.options.core.is_redraw_annos_triggered = true;
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("close").clicked() {
            window_open = false;
        }
    });
    Ok(ToolsData {
        specifics: ToolSpecifics::Bbox(data),
        menu_active: window_open,
        visible_inactive_tools,
    })
}

pub fn brush_menu(
    ui: &mut Ui,
    mut window_open: bool,
    mut data: BrushToolData,
    are_tools_active: &mut bool,
    mut visible_inactive_tools: VisibleInactiveToolsState,
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
        data.options.core = data.options.core.trigger_redraw_and_hist();
    }
    if show_only_change {
        data.options.core.is_redraw_annos_triggered = true;
    }

    ui.separator();
    data.options.core = toggle_erase(ui, data.options.core);
    data.options.core = hide_menu(ui, data.options.core);
    ui.checkbox(&mut data.options.core.auto_paste, "auto paste");
    egui::CollapsingHeader::new("advanced").show(ui, |ui| {
        ui.checkbox(&mut data.options.core.track_changes, "track changes");
        ui.separator();
        ui.label("properties");
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
        ui.separator();
        ui.label("visualization");
        if transparency_slider(
            ui,
            are_tools_active,
            &mut data.options.fill_alpha,
            "transparency",
        ) {
            data.options.core.is_redraw_annos_triggered = true;
        }
        if ui.button("new random colors").clicked() {
            data.options.core.is_colorchange_triggered = true;
        }
        ui.separator();
        ui.checkbox(
            &mut data.options.per_file_crowd,
            "export merged annotations per file",
        );
        let skip_import_mode = false;
        trace_ok_err(export_file_menu(
            ui,
            "coco file",
            &mut data.coco_file,
            are_tools_active,
            &mut data.options.core.import_export_trigger,
            skip_import_mode,
        ));
    });
    ui.separator();
    if show_inactive_tool_menu(ui, BRUSH_NAME, &mut visible_inactive_tools) {
        data.options.core.is_redraw_annos_triggered = true;
    }
    ui.separator();
    if ui.button("close").clicked() {
        window_open = false;
    }
    Ok(ToolsData {
        specifics: ToolSpecifics::Brush(data),
        menu_active: window_open,
        visible_inactive_tools,
    })
}

fn update_numeric_attribute<T>(
    ui: &mut Ui,
    are_tools_active: &mut bool,
    x: &mut Option<T>,
    attr_type_label: &str,
    new_attr_buffer: &mut String,
) -> bool
where
    T: Display + FromStr + Debug,
    <T as FromStr>::Err: Debug,
{
    let (input_changed, new_val) =
        process_number(ui, are_tools_active, attr_type_label, new_attr_buffer);
    if let Some(new_val) = new_val {
        *x = Some(new_val);
    }
    if new_attr_buffer.is_empty() {
        *x = None;
    }
    input_changed
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
    ui.horizontal(|ui| {
        egui::ComboBox::from_label("")
            .selected_text(format!(
                "{:?}",
                match data.new_attr_val {
                    AttrVal::Float(_) => FLOAT_LABEL,
                    AttrVal::Int(_) => INT_LABEL,
                    AttrVal::Str(_) => TEXT_LABEL,
                    AttrVal::Bool(_) => BOOL_LABEL,
                }
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut data.new_attr_val, AttrVal::Float(None), FLOAT_LABEL);
                ui.selectable_value(&mut data.new_attr_val, AttrVal::Int(None), INT_LABEL);
                ui.selectable_value(
                    &mut data.new_attr_val,
                    AttrVal::Str(String::new()),
                    TEXT_LABEL,
                );
                ui.selectable_value(&mut data.new_attr_val, AttrVal::Bool(false), BOOL_LABEL);
            });
    });
    text_edit_singleline(ui, &mut data.new_attr_name, are_tools_active);
    if ui.button("add").clicked() {
        if data.attr_names().contains(&data.new_attr_name) {
            tracing::error!(
                "attribute {:?} already exists, we do not re-create it",
                data.new_attr_name
            );
        } else {
            data.options.is_addition_triggered = true;
            data.options.is_update_triggered = true;
        }
    }
    egui::Grid::new("attributes_grid")
        .num_columns(4)
        .show(ui, |ui| {
            let n_rows = data.attr_names().len();
            let to_be_removed = removable_rows(ui, n_rows, |ui, idx_row| {
                let attr_name = data.attr_names()[idx_row].clone();
                let mut new_attr_buffer = mem::take(data.attr_value_buffer_mut(idx_row));
                ui.label(&attr_name);
                let attr_map = &mut data.current_attr_map;
                let mut input_changed = false;
                if let Some(attr_map) = attr_map {
                    match attr_map.get_mut(&attr_name) {
                        Some(AttrVal::Bool(b)) => {
                            if ui.checkbox(b, "").changed() {
                                input_changed = true;
                            }
                        }
                        Some(AttrVal::Float(x)) => {
                            input_changed = update_numeric_attribute(
                                ui,
                                are_tools_active,
                                x,
                                FLOAT_LABEL,
                                &mut new_attr_buffer,
                            );
                        }
                        Some(AttrVal::Int(x)) => {
                            let lost_focus = update_numeric_attribute(
                                ui,
                                are_tools_active,
                                x,
                                INT_LABEL,
                                &mut new_attr_buffer,
                            );
                            if lost_focus {
                                input_changed = true;
                            }
                        }
                        Some(AttrVal::Str(s)) => {
                            let lost_focus = text_edit_singleline(ui, s, are_tools_active)
                                .on_hover_text(TEXT_LABEL)
                                .lost_focus();
                            if lost_focus {
                                input_changed = true;
                            }
                        }
                        None => {
                            warn!("attr_map does not contain {attr_name}");
                        }
                    }
                    if input_changed {
                        data.to_propagate_attr_val
                            .retain(|(idx_attr, _)| *idx_attr != idx_row);
                        data.options.is_update_triggered = true;
                    }

                    let checked = data
                        .to_propagate_attr_val
                        .iter()
                        .any(|(idx_attr, _)| *idx_attr == idx_row);

                    if ui
                        .selectable_label(checked, "propagate")
                        .on_hover_text("propaget attribute value to next opened image")
                        .clicked()
                    {
                        if checked {
                            data.to_propagate_attr_val
                                .retain(|(idx_attr, _)| *idx_attr != idx_row);
                        } else {
                            data.to_propagate_attr_val
                                .push((idx_row, attr_map[&attr_name].clone()));
                        }
                    }
                    *data.attr_value_buffer_mut(idx_row) = new_attr_buffer;
                }

                if ui.button("rename").clicked() {
                    data.options.rename_src_idx = Some(idx_row);
                    data.options.is_update_triggered = true;
                }
                ui.end_row();
            });
            if let Some(tbr) = to_be_removed {
                data.options.removal_idx = Some(tbr);
            }
        });

    ui.separator();
    let skip_merge_menu = true;
    ui.checkbox(
        &mut data.options.export_only_opened_folder,
        "export only opened folder",
    );
    export_file_menu(
        ui,
        "export attributes as json",
        &mut data.export_path,
        are_tools_active,
        &mut data.options.import_export_trigger,
        skip_merge_menu,
    )?;

    ui.separator();
    if ui.button("Close").clicked() {
        window_open = false;
    }

    Ok(ToolsData {
        specifics: ToolSpecifics::Attributes(data),
        menu_active: window_open,
        visible_inactive_tools: VisibleInactiveToolsState::default(),
    })
}
