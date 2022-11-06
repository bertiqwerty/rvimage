use egui::Ui;

use crate::tools_data::{
    bbox_data::{BboxExportFileType, BboxToolData},
    ToolSpecifics, ToolsData,
};

pub fn bbox_menu(ui: &mut Ui, mut window_open: bool, mut data: BboxToolData) -> ToolsData {
    let mut new_idx = data.cat_id_current;
    let mut new_label = None;
    if ui.text_edit_singleline(&mut data.new_label).lost_focus() {
        new_label = Some(data.new_label.clone());
    }
    let empty = data.find_empty();
    if let (Some(empty), Some(new_label)) = (empty, new_label.as_ref()) {
        *empty = new_label.clone();
    } else if let Some(new_label) = new_label {
        data.push(new_label, None);
        new_idx = data.len() - 1;
    }
    let mut to_be_removed = None;
    for (label_idx, label) in data.labels().iter().enumerate() {
        let checked = label_idx == data.cat_id_current;
        ui.horizontal_top(|ui| {
            if ui.button("x").clicked() {
                to_be_removed = Some(label_idx);
            }
            if ui.selectable_label(checked, label).clicked() {
                new_idx = label_idx;
            }
        });
    }
    if new_idx != data.cat_id_current {
        for (_, anno) in data.anno_iter_mut() {
            anno.label_selected(new_idx);
        }
        data.cat_id_current = new_idx;
    }
    if let Some(idx) = to_be_removed {
        data.remove_cat(idx);
    }
    if ui.button("export pickle").clicked() {
        data.export_file_type = BboxExportFileType::PICKLE;
        data.write_label_file = true;
    }
    if ui.button("export json").clicked() {
        data.export_file_type = BboxExportFileType::JSON;
        data.write_label_file = true;
    }
    if ui.button("Close").clicked() {
        window_open = false;
    }
    ToolsData {
        specifics: ToolSpecifics::Bbox(data),
        menu_active: window_open,
    }
}
