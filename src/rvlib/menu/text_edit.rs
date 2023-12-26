use egui::{Response, Ui};

pub fn text_edit(
    text: &mut String,
    are_tools_active: &mut bool,
    mut f_text_edit: impl FnMut(&mut String) -> Response,
) -> Response {
    let filter_txt_field = f_text_edit(text);
    *are_tools_active = if filter_txt_field.gained_focus() {
        false
    } else if filter_txt_field.lost_focus() {
        true
    } else {
        *are_tools_active
    };
    filter_txt_field
}
pub fn text_edit_singleline(
    ui: &mut Ui,
    text: &mut String,
    are_tools_active: &mut bool,
) -> Response {
    text_edit(text, are_tools_active, |text| ui.text_edit_singleline(text))
}
