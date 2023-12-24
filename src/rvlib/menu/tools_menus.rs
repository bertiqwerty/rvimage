use std::{path::PathBuf, str::FromStr};

use egui::Ui;

use crate::{
    cfg::{self, get_cfg, CocoFileConnection},
    file_util::path_to_str,
    result::{to_rv, RvResult},
    tools_data::{
        annotations::SplitMode, bbox_data::BboxSpecificData, BrushToolData, LabelInfo,
        ToolSpecifics, ToolsData, OUTLINE_THICKNESS_CONVERSION,
    },
};

pub fn label_menu(
    ui: &mut Ui,
    label_info: &mut LabelInfo,
) -> RvResult<(usize, Option<usize>)> {
    let mut new_idx = label_info.cat_idx_current;
    let mut new_label = None;
    if ui
        .text_edit_singleline(&mut label_info.new_label)
        .lost_focus()
    {
        new_label = Some(label_info.new_label.clone());
    }
    let default_label = label_info.find_default();
    if let (Some(default_label), Some(new_label)) = (default_label, new_label.as_ref()) {
        *default_label = new_label.clone();
    } else if let Some(new_label) = new_label {
        label_info.push(new_label, None, None)?;
        new_idx = label_info.len() - 1;
    }
    let mut to_be_removed = None;
    for (label_idx, label) in label_info.labels().iter().enumerate() {
        let checked = label_idx == label_info.cat_idx_current;
        ui.horizontal_top(|ui| {
            if ui.button("x").clicked() {
                to_be_removed = Some(label_idx);
            }
            if ui.selectable_label(checked, label).clicked() {
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
    Ok((new_idx, to_be_removed))
}

pub fn bbox_menu(
    ui: &mut Ui,
    mut window_open: bool,
    mut data: BboxSpecificData,
) -> RvResult<ToolsData> {
    let (new_idx, to_be_removed) = label_menu(ui, &mut data.label_info)?;
    if new_idx != data.label_info.cat_idx_current {
        for (_, (anno, _)) in data.anno_iter_mut() {
            anno.label_selected(new_idx);
        }
        data.label_info.cat_idx_current = new_idx;
    }
    if let Some(idx) = to_be_removed {
        data.remove_catidx(idx);
    }
    ui.separator();
    let mut pathincfg_triggered = false;

    let mut hide_boxes = !data.options.are_boxes_visible;
    if ui.checkbox(&mut hide_boxes, "hide boxes").clicked() {
        data.options.is_redraw_annos_triggered = true;
    }
    data.options.are_boxes_visible = !hide_boxes;

    ui.checkbox(&mut data.options.auto_paste, "auto paste");

    let mut txt = path_to_str(&data.coco_file.path)?.to_string();
    egui::CollapsingHeader::new("advanced").show(ui, |ui| {
        let mut transparency: f32 = data.options.fill_alpha as f32 / 255.0 * 100.0;
        ui.label("transparency");
        if ui
            .add(egui::Slider::new(&mut transparency, 0.0..=100.0).text("fill"))
            .changed()
        {
            data.options.is_redraw_annos_triggered = true;
        }
        data.options.fill_alpha = (transparency / 100.0 * 255.0).round() as u8;
        let mut transparency: f32 = data.options.outline_alpha as f32 / 255.0 * 100.0;
        if ui
            .add(egui::Slider::new(&mut transparency, 0.0..=100.0).text("outline"))
            .changed()
        {
            data.options.is_redraw_annos_triggered = true;
        }
        data.options.outline_alpha = (transparency / 100.0 * 255.0).round() as u8;
        let mut outline_thickness_f =
            data.options.outline_thickness as f32 / OUTLINE_THICKNESS_CONVERSION;
        ui.separator();
        if ui
            .add(egui::Slider::new(&mut outline_thickness_f, 0.0..=10.0).text("outline thickness"))
            .changed()
        {
            data.options.is_redraw_annos_triggered = true;
        }
        data.options.outline_thickness =
            (outline_thickness_f * OUTLINE_THICKNESS_CONVERSION).round() as u16;

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
        ui.horizontal(|ui| {
            ui.label("coco file");
            ui.radio_value(&mut data.coco_file.conn, CocoFileConnection::Local, "local");
            ui.radio_value(&mut data.coco_file.conn, CocoFileConnection::Ssh, "ssh");
            ui.text_edit_singleline(&mut txt);
        });
        if ui.button("store path in cfg").clicked() {
            pathincfg_triggered = true;
        }
        ui.separator();
        if ui.button("new random colors").clicked() {
            data.options.is_colorchange_triggered = true;
        }
        if ui.button("clear out of folder annotations").clicked() {
            data.options.is_anno_rm_triggered = true;
        }
    });
    if path_to_str(&data.coco_file.path)? != txt {
        data.coco_file.path = PathBuf::from_str(&txt).map_err(to_rv)?;
    }
    if pathincfg_triggered {
        tracing::info!("saving coco path to cfg file");
        let mut curcfg = get_cfg()?;
        curcfg.coco_file = Some(data.coco_file.clone());
        cfg::write_cfg(&curcfg)?;
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("export coco").clicked() {
            tracing::info!("export coco triggered");
            data.options.is_export_triggered = true;
            pathincfg_triggered = true;
        }
        if ui.button("import coco").clicked() {
            tracing::info!("import triggered");
            data.options.is_coco_import_triggered = true;
            pathincfg_triggered = true;
        }
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
) -> RvResult<ToolsData> {
    let (new_idx, to_be_removed) = label_menu(ui, &mut data.label_info)?;
    ui.add(egui::Slider::new(&mut data.options.thickness, 0.0..=20.0).text("thickness"))
        .changed();
    ui.add(egui::Slider::new(&mut data.options.intensity, 0.0..=1.0).text("intensity"))
        .changed();
    if ui.button("close").clicked() {
        window_open = false;
    }
    Ok(ToolsData {
        specifics: ToolSpecifics::Brush(data),
        menu_active: window_open,
    })
}
