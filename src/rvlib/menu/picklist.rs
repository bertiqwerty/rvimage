use egui::{Id, Response, Ui};

enum ListPopupResult {
    ElementIndex(usize),
    Cancel,
}

pub enum PicklistResult {
    Picked(String),
    Cancel,
}
fn show_list_popup<'a, I>(
    ui: &mut Ui,
    folders: I,
    popup_id: Id,
    min_width: f32,
    below_respone: &Response,
) -> Option<ListPopupResult>
where
    I: Iterator<Item = &'a str>,
{
    ui.memory_mut(|m| m.open_popup(popup_id));
    let mut selected_idx = None;
    egui::popup_below_widget(ui, popup_id, below_respone, |ui| {
        ui.set_min_width(min_width);
        for (i, f) in folders.enumerate() {
            if ui.button(f).clicked() {
                selected_idx = Some(ListPopupResult::ElementIndex(i));
            }
        }
        if ui.button("cancel").clicked() {
            selected_idx = Some(ListPopupResult::Cancel);
        }
    });
    selected_idx
}

pub fn pick<'a, I>(
    ui: &mut Ui,
    mut elt_iter: I,
    min_width: f32,
    response: &Response,
    popup_str: &str,
) -> Option<PicklistResult>
where
    I: Iterator<Item = &'a str> + Clone,
{
    let popup_id = ui.make_persistent_id(popup_str);
    let idx = show_list_popup(ui, elt_iter.clone(), popup_id, min_width, response);

    match idx {
        Some(ListPopupResult::ElementIndex(idx)) => elt_iter
            .nth(idx)
            .map(|elt| (PicklistResult::Picked(elt.to_string()))),
        Some(ListPopupResult::Cancel) => Some(PicklistResult::Cancel),
        _ => None,
    }
}
