use egui::{Response, Ui};

pub fn text_edit_singleline(
    ui: &mut Ui,
    text: &mut String,
    are_tools_active: &mut bool,
) -> Response {
    let filter_txt_field = ui.text_edit_singleline(text);
    *are_tools_active = if filter_txt_field.gained_focus() {
        false
    } else if filter_txt_field.lost_focus() {
        true
    } else {
        *are_tools_active
    };
    filter_txt_field
}
