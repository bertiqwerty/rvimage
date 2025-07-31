use egui::{Popup, Response};

fn show_list_popup<'a, I>(folders: I, min_width: f32, btn_response: &Response) -> Option<usize>
where
    I: Iterator<Item = &'a str>,
{
    tracing::warn!("BTN RESP {btn_response:?}");
    let mut selected_idx = None;

    Popup::menu(btn_response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            tracing::warn!("SHOW MENU");
            for (i, f) in folders.enumerate() {
                tracing::warn!("folder {f} {min_width}");
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

pub fn pick<'a, I>(mut elt_iter: I, min_width: f32, response: &Response) -> Option<String>
where
    I: Iterator<Item = &'a str> + Clone,
{
    let idx = show_list_popup(elt_iter.clone(), min_width, response);

    match idx {
        Some(idx) => elt_iter.nth(idx).map(|elt| (elt.to_string())),
        _ => None,
    }
}
