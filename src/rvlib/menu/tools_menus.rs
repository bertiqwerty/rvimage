use egui::Ui;

use crate::{
    result::RvResult,
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
    ui.separator();
    if ui.button("clear out of folder annotations").clicked() {
        data.flags.is_anno_rm_triggered = true;
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("export coco").clicked() {
            println!("export coco triggered");
            data.export_trigger.is_export_triggered = true;
        }
        if ui.button("import coco").clicked() {
            println!("import triggered");
            data.flags.is_coco_import_triggered = true;
        }
    });
    ui.separator();
    ui.checkbox(&mut data.flags.auto_paste, "auto paste");
    ui.separator();
    if ui.button("close").clicked() {
        window_open = false;
    }
    Ok(ToolsData {
        specifics: ToolSpecifics::Bbox(data),
        menu_active: window_open,
    })
}
