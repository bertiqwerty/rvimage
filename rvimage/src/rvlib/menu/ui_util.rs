use std::{
    fmt::{Debug, Display},
    ops::RangeInclusive,
    str::FromStr,
};

use egui::{
    text::{CCursor, CCursorRange},
    FontSelection, Response, TextBuffer, TextEdit, Ui,
};
use tracing::warn;

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
pub fn text_edit_singleline(
    ui: &mut Ui,
    text: &mut String,
    are_tools_active: &mut bool,
) -> Response {
    text_edit_with_deactivated_tools(text, are_tools_active, |text| {
        let mut textedit_output = TextEdit::singleline(text)
            .font(FontSelection::Style(egui::TextStyle::Monospace))
            .show(ui);
        if textedit_output.response.clicked() {
            textedit_output
                .state
                .cursor
                .set_char_range(Some(CCursorRange::two(
                    CCursor::new(0),
                    CCursor::new(text.len()),
                )));
            textedit_output
                .state
                .store(ui.ctx(), textedit_output.response.id);
        }
        textedit_output.response
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

pub fn process_number<T>(
    ui: &mut Ui,
    are_tools_active: &mut bool,
    label: &str,
    buffer: &mut String,
) -> (bool, Option<T>)
where
    T: Display + FromStr,
    <T as FromStr>::Err: Debug,
{
    let new_val = text_edit_singleline(ui, buffer, are_tools_active).on_hover_text(label);
    if new_val.changed() {
        match buffer.parse::<T>() {
            Ok(val) => (true, Some(val)),
            Err(e) => {
                warn!("could not parse '{buffer}' as number due to {e:?}");
                (false, None)
            }
        }
    } else {
        (false, None)
    }
}
pub fn button_triggerable_number<T>(
    ui: &mut Ui,
    buffer: &mut String,
    are_tools_active: &mut bool,
    btn_label: &str,
    tool_tip: &str,
    warning_tool_tip_btn: Option<&str>,
) -> Option<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Debug,
{
    let _ = process_number::<T>(ui, are_tools_active, tool_tip, buffer);
    let btn = ui.button(btn_label);
    let clicked = if let Some(warning) = warning_tool_tip_btn {
        btn.on_hover_text(warning).double_clicked()
    } else {
        btn.clicked()
    };
    if clicked {
        buffer
            .parse::<T>()
            .inspect_err(|e| tracing::warn!("could not parse '{buffer}' as number due to {e:?}"))
            .ok()
    } else {
        None
    }
}
