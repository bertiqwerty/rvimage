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
    confirmation_msgs: Option<(&str, &str)>,
) -> Option<T>
where
    T: Display + FromStr,
    <T as FromStr>::Err: Debug,
{
    let _ = process_number::<T>(ui, are_tools_active, tool_tip, buffer);
    let clicked = if let Some((title, msg)) = confirmation_msgs {
        button_confirmed(ui, btn_label, title, msg)
    } else {
        ui.button(btn_label).clicked()
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

fn nth_row_idx(idx: usize) -> String {
    let idx_based_on_1 = idx + 1;
    let nth = match idx_based_on_1 % 100 {
        11..=13 => "th",
        _ => match idx_based_on_1 % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{}{nth}", idx_based_on_1)
}

pub fn removable_rows(
    ui: &mut Ui,
    n_rows: usize,
    mut make_row: impl FnMut(&mut Ui, usize),
) -> Option<usize> {
    let mut to_be_removed = None;
    for idx in 0..n_rows {
        let msg = format!("Are you sure to delete the {} row?", nth_row_idx(idx));
        if button_confirmed(ui, "x", "Delete", msg) {
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

pub fn button_confirmed<'a>(
    ui: &mut egui::Ui,
    button_txt: impl egui::IntoAtoms<'a>,
    modal_title: impl Into<egui::RichText> + Debug,
    modal_txt: impl Into<egui::WidgetText> + Debug,
) -> bool {
    let modal_id = ui.make_persistent_id(format!(
        "auto_inline_modal, {:?}, {:?}",
        &modal_title, &modal_txt
    ));

    let button_clicked = ui.button(button_txt).clicked();

    let ctx = ui.ctx();
    let mut confirmed = false;

    if button_clicked {
        ctx.data_mut(|data| data.insert_temp(modal_id, true));
    }

    let is_open: bool = ctx.data(|data| data.get_temp(modal_id).unwrap_or(false));

    if is_open {
        egui::Modal::new(modal_id).show(ctx, |ui| {
            ui.heading(modal_title);
            ui.label(modal_txt);

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button("Okay").clicked() {
                    confirmed = true;
                    ui.ctx().data_mut(|data| data.insert_temp(modal_id, false));
                }
                if ui.button("Cancel").clicked() {
                    ui.ctx().data_mut(|data| data.insert_temp(modal_id, false));
                }
            });
        });
    }

    confirmed
}

#[test]
fn test_nth() {
    assert_eq!(nth_row_idx(0), "1st".to_string());
    assert_eq!(nth_row_idx(1), "2nd".to_string());
    assert_eq!(nth_row_idx(2), "3rd".to_string());
    assert_eq!(nth_row_idx(3), "4th".to_string());
    assert_eq!(nth_row_idx(111), "112th".to_string());
    assert_eq!(nth_row_idx(10), "11th".to_string());
    assert_eq!(nth_row_idx(12), "13th".to_string());
    assert_eq!(nth_row_idx(11120), "11121st".to_string());
}
