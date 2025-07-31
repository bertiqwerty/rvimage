use egui::{Popup, Response};

fn show_list_popup<'a, I>(folders: I, btn_response: &Response) -> Option<usize>
where
    I: Iterator<Item = &'a str>,
{
    let mut selected_idx = None;

    Popup::menu(btn_response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            for (i, f) in folders.enumerate() {
                if ui.button(f).clicked() {
                    selected_idx = Some(i);
                }
            }
            if ui.button("cancel").clicked() {
                ui.close();
            }
        });
    selected_idx
}

pub fn pick<'a, I>(mut elt_iter: I, response: &Response) -> Option<String>
where
    I: Iterator<Item = &'a str> + Clone,
{
    let idx = show_list_popup(elt_iter.clone(), response);

    match idx {
        Some(idx) => elt_iter.nth(idx).map(|elt| (elt.to_string())),
        _ => None,
    }
}
