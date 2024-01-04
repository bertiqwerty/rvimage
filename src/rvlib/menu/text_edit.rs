use egui::{FontSelection, Response, TextBuffer, TextEdit, Ui, Widget};

pub fn text_edit<S: TextBuffer>(
    text: &mut S,
    are_tools_active: &mut bool,
    mut f_text_edit: impl FnMut(&mut S) -> Response,
) -> Response {
    let txt_field = f_text_edit(text);
    *are_tools_active = if txt_field.gained_focus() {
        false
    } else if txt_field.lost_focus() {
        true
    } else {
        *are_tools_active
    };
    txt_field
}
pub fn text_edit_singleline<S: TextBuffer>(
    ui: &mut Ui,
    text: &mut S,
    are_tools_active: &mut bool,
) -> Response {
    text_edit(text, are_tools_active, |text| {
        TextEdit::singleline(text)
            .font(FontSelection::Style(egui::TextStyle::Monospace))
            .ui(ui)
    })
}
