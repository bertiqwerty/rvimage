use std::{
    fmt::{Debug, Display},
    ops::RangeInclusive,
    str::FromStr,
};

use egui::{
    FontSelection, Response, TextBuffer, TextEdit, Ui,
    text::{CCursor, CCursorRange},
};
use tracing::warn;

fn ui_with_deactivated_tools(
    are_tools_active: &mut bool,
    mut f_ui: impl FnMut() -> Response,
    event_activate: impl Fn(&Response) -> bool,
    event_deactivate: impl Fn(&Response) -> bool,
) -> Response {
    let response = f_ui();
    *are_tools_active = if event_deactivate(&response) {
        false
    } else if event_activate(&response) {
        true
    } else {
        *are_tools_active
    };
    response
}

pub fn ui_with_deactivated_tools_on_keys(
    are_tools_active: &mut bool,
    f_ui: impl FnMut() -> Response,
) -> Response {
    ui_with_deactivated_tools(
        are_tools_active,
        f_ui,
        |response| response.lost_focus(),
        |response| response.gained_focus(),
    )
}
pub fn ui_with_deactivated_tools_on_hover(
    are_tools_active: &mut bool,
    f_ui: impl FnMut() -> Response,
) -> Response {
    ui_with_deactivated_tools(
        are_tools_active,
        f_ui,
        |response| !response.hovered(),
        |response| response.hovered(),
    )
}

pub fn text_edit_with_deactivated_tools<S: TextBuffer>(
    text: &mut S,
    are_tools_active: &mut bool,
    mut f_ui: impl FnMut(&mut S) -> Response,
) -> Response {
    ui_with_deactivated_tools_on_keys(are_tools_active, || f_ui(text))
}
pub fn text_edit_multiline(
    ui: &mut Ui,
    text: &mut String,
    are_tools_active: &mut bool,
) -> Response {
    text_edit_with_deactivated_tools(text, are_tools_active, |text| {
        TextEdit::multiline(text)
            .font(FontSelection::Style(egui::TextStyle::Monospace))
            .show(ui)
            .response
            .response
    })
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
        textedit_output.response.response
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
    ui_with_deactivated_tools_on_keys(are_tools_active, || {
        ui.add(egui::Slider::new(value, range.clone()).text(text))
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
                if !buffer.is_empty() {
                    warn!("could not parse '{buffer}' as number due to {e:?}");
                    (false, None)
                } else {
                    (true, None)
                }
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

pub fn removable_rows(
    ui: &mut Ui,
    n_rows: usize,
    mut make_row: impl FnMut(&mut Ui, usize),
) -> Option<usize> {
    let mut to_be_removed = None;
    for idx in 0..n_rows {
        if ui
            .button("x")
            .on_hover_text("double click😈")
            .double_clicked()
        {
            to_be_removed = Some(idx);
        }
        make_row(ui, idx)
    }
    to_be_removed
}

pub fn update_numeric_attribute<T>(
    ui: &mut Ui,
    are_tools_active: &mut bool,
    x: &mut Option<T>,
    attr_type_label: &str,
    new_attr_buffer: &mut String,
) -> bool
where
    T: Display + FromStr + Debug,
    <T as FromStr>::Err: Debug,
{
    let (input_changed, new_val) =
        process_number(ui, are_tools_active, attr_type_label, new_attr_buffer);
    if let Some(new_val) = new_val {
        *x = Some(new_val);
    }
    if new_attr_buffer.is_empty() {
        *x = None;
    }
    input_changed
}
