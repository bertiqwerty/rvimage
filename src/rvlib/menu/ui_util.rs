use std::ops::RangeInclusive;

use egui::{FontSelection, Response, TextBuffer, TextEdit, Ui, Widget};

pub fn ui_with_deactivated_tools(
    are_tools_active: &mut bool,
    mut f_ui: impl FnMut() -> Response,
) -> Response {
    let response = f_ui();
    *are_tools_active = if response.gained_focus() {
        false
    } else if response.lost_focus() {
        true
    } else {
        *are_tools_active
    };
    response
}

pub fn text_edit_with_deactivated_tools<S: TextBuffer>(
    text: &mut S,
    are_tools_active: &mut bool,
    mut f_ui: impl FnMut(&mut S) -> Response,
) -> Response {
    ui_with_deactivated_tools(are_tools_active, || f_ui(text))
}
pub fn text_edit_singleline<S: TextBuffer>(
    ui: &mut Ui,
    text: &mut S,
    are_tools_active: &mut bool,
) -> Response {
    text_edit_with_deactivated_tools(text, are_tools_active, |text| {
        TextEdit::singleline(text)
            .font(FontSelection::Style(egui::TextStyle::Monospace))
            .ui(ui)
    })
}
pub fn slider<Num>(
    ui: &mut Ui,
    are_tools_active: &mut bool,
    value: &mut Num,
    range: RangeInclusive<Num>,
    text: &str,
) -> Response
where
    Num: egui::emath::Numeric,
{
    ui_with_deactivated_tools(are_tools_active, || {
        let slider = ui.add(egui::Slider::new(value, range.clone()).text(text));
        slider
    })
}
