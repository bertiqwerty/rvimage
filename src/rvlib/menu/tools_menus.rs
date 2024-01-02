use std::{collections::HashMap, path::PathBuf, str::FromStr};

use egui::Ui;
use tracing::{info, warn};

use crate::{
    cfg::{ExportPath, ExportPathConnection},
    domain::{Annotate, TPtF},
    file_util::path_to_str,
    result::{to_rv, RvResult},
    tools_data::{
        annotations::{InstanceAnnotations, SplitMode},
        bbox_data::BboxSpecificData,
        BrushToolData, CoreOptions, LabelInfo, ToolSpecifics, ToolsData,
        OUTLINE_THICKNESS_CONVERSION,
    },
    ShapeI,
};

use super::text_edit::text_edit_singleline;

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
    let mut new_label = None;
    let mut label_change = false;
    let mut show_only_change = false;

    let label_field = text_edit_singleline(ui, &mut label_info.new_label, are_tools_active);
    if label_field.lost_focus() {
        new_label = Some(label_info.new_label.clone());
    }
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
                egui::RichText::new(label).strong().italics()
            } else {
                egui::RichText::new(label)
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
        if let Some(is_import_triggered) = is_import_triggered {
            if ui.button("import").clicked() {
                tracing::info!("import triggered");
                *is_import_triggered = true;
            }
        }
    });
    Ok(())
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

    data.options.core_options = hide_menu(ui, data.options.core_options);

    ui.checkbox(&mut data.options.auto_paste, "auto paste");

    let mut export_file_menu_result = Ok(());
    egui::CollapsingHeader::new("advanced").show(ui, |ui| {
        let mut transparency: f32 = data.options.fill_alpha as f32 / 255.0 * 100.0;
        ui.label("transparency");
        if ui
            .add(egui::Slider::new(&mut transparency, 0.0..=100.0).text("fill"))
            .changed()
        {
            data.options.core_options.is_redraw_annos_triggered = true;
        }
        data.options.fill_alpha = (transparency / 100.0 * 255.0).round() as u8;
        let mut transparency = data.options.outline_alpha as f32 / 255.0 * 100.0;
        if ui
            .add(egui::Slider::new(&mut transparency, 0.0..=100.0).text("outline"))
            .changed()
        {
            data.options.core_options.is_redraw_annos_triggered = true;
        }
        data.options.outline_alpha = (transparency / 100.0 * 255.0).round() as u8;
        let mut outline_thickness_f =
            data.options.outline_thickness as TPtF / OUTLINE_THICKNESS_CONVERSION;
        ui.separator();
        if ui
            .add(egui::Slider::new(&mut outline_thickness_f, 0.0..=10.0).text("outline thickness"))
            .changed()
        {
            data.options.core_options.is_redraw_annos_triggered = true;
        }
        data.options.outline_thickness =
            (outline_thickness_f * OUTLINE_THICKNESS_CONVERSION).round() as u16;

        ui.separator();
        if ui
            .add(
                egui::Slider::new(&mut data.options.drawing_distance, 1..=50)
                    .text("drawing distance parameter"),
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
            Some(&mut data.options.is_coco_import_triggered),
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

    data.options.core_options = hide_menu(ui, data.options.core_options);
    if ui
        .add(egui::Slider::new(&mut data.options.thickness, 0.0..=250.0).text("thickness"))
        .changed()
    {
        data.options.is_selection_change_needed = true;
    }
    if ui
        .add(egui::Slider::new(&mut data.options.intensity, 0.0..=1.0).text("intensity"))
        .changed()
    {
        data.options.is_selection_change_needed = true;
    }
    if ui.checkbox(&mut data.options.erase, "erase").clicked() {
        if data.options.erase {
            info!("start erasing");
        } else {
            info!("stop erasing");
        }
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
    )?;
    if ui.button("close").clicked() {
        window_open = false;
    }
    Ok(ToolsData {
        specifics: ToolSpecifics::Brush(data),
        menu_active: window_open,
    })
}
