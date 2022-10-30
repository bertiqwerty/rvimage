use egui::Ui;

use crate::tools_menus_data::{BboxSpecifics, ToolSpecifics, ToolsMenuData};

pub fn bbox_menu(ui: &mut Ui, mut window_open: bool, mut data: BboxSpecifics) -> ToolsMenuData {
    if ui.text_edit_singleline(&mut data.new_label).lost_focus() {
        data.push(data.new_label.clone());
    }
    let mut to_be_removed = None;
    let mut new_idx = data.idx_current;
    for (i, label) in data.labels().iter().enumerate() {
        let checked = i == data.idx_current;
        ui.horizontal_top(|ui| {
            if ui.selectable_label(checked, label).clicked() {
                new_idx = i;
            }
            if ui.button("del").clicked() {
                to_be_removed = Some(i);
            }
        });
    }
    data.idx_current = new_idx;
    if let Some(idx) = to_be_removed {
        data.remove(idx);
    }
    if ui.button("Close").clicked() {
        window_open = false;
    }
    ToolsMenuData {
        specifics: ToolSpecifics::Bbox(data),
        menu_active: window_open,
    }
}
