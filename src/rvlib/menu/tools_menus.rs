use egui::Ui;

use crate::tools_menus_data::{ToolSpecifics, ToolsMenuData};

pub fn bbox_menu(ui: &mut Ui, window_open: bool, data: String) -> ToolsMenuData {
    ui.button("bbox menu").clicked();
    ToolsMenuData {
        specifics: ToolSpecifics::Bbox(data),
        menu_active: window_open,
    }
}
