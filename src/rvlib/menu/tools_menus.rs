use std::{path::PathBuf, str::FromStr};

use egui::Ui;

use crate::{
    annotations::SplitMode,
    cfg::{self, get_cfg, CocoFileConnection},
    file_util::path_to_str,
    result::{to_rv, RvResult},
    tools_data::{bbox_data::BboxSpecificData, ToolSpecifics, ToolsData},
};

pub fn bbox_menu(
    ui: &mut Ui,
    mut window_open: bool,
    mut data: BboxSpecificData,
) -> RvResult<ToolsData> {
    let mut new_idx = data.cat_idx_current;
    let mut new_label = None;
    if ui.text_edit_singleline(&mut data.new_label).lost_focus() {
        new_label = Some(data.new_label.clone());
    }
    let default_label = data.find_default();
    if let (Some(default_label), Some(new_label)) = (default_label, new_label.as_ref()) {
        *default_label = new_label.clone();
    } else if let Some(new_label) = new_label {
        data.push(new_label, None, None)?;
        new_idx = data.len() - 1;
    }
    let mut to_be_removed = None;
    for (label_idx, label) in data.labels().iter().enumerate() {
        let checked = label_idx == data.cat_idx_current;
        ui.horizontal_top(|ui| {
            if ui.button("x").clicked() {
                to_be_removed = Some(label_idx);
            }
            if ui.selectable_label(checked, label).clicked() {
                new_idx = label_idx;
            }
            let rgb = data.colors()[label_idx];
            ui.label(
                egui::RichText::new("■")
                    .heading()
                    .strong()
                    .color(egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2])),
            );
        });
    }
    if new_idx != data.cat_idx_current {
        for (_, (anno, _)) in data.anno_iter_mut() {
            anno.label_selected(new_idx);
        }
        data.cat_idx_current = new_idx;
    }
    if let Some(idx) = to_be_removed {
        data.remove_catidx(idx);
    }
    let mut pathincfg_triggered = false;
    ui.separator();

    let mut hide_boxes = !data.options.are_boxes_visible;
    if ui.checkbox(&mut hide_boxes, "hide boxes").clicked() {
        data.options.is_redraw_annos_triggered = true;
    }
    data.options.are_boxes_visible = !hide_boxes;

    ui.checkbox(&mut data.options.auto_paste, "auto paste");

    let mut txt = path_to_str(&data.coco_file.path)?.to_string();
    egui::CollapsingHeader::new("Advanced").show(ui, |ui| {
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
        println!("saving coco path to cfg file");
        let mut curcfg = get_cfg()?;
        curcfg.coco_file = Some(data.coco_file.clone());
        cfg::write_cfg(&curcfg)?;
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("export coco").clicked() {
            println!("export coco triggered");
            data.options.is_export_triggered = true;
            pathincfg_triggered = true;
        }
        if ui.button("import coco").clicked() {
            println!("import triggered");
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
