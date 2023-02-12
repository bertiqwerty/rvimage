use egui::{Id, Response, Ui};

fn show_list_popup<'a, I>(
    ui: &mut Ui,
    folders: I,
    popup_id: Id,
    min_width: f32,
    below_respone: &Response,
) -> Option<usize>
where
    I: Iterator<Item = &'a str>,
{
    ui.memory_mut(|m| m.open_popup(popup_id));
    let mut selected_idx = None;
    egui::popup_below_widget(ui, popup_id, below_respone, |ui| {
        ui.set_min_width(min_width);
        for (i, f) in folders.enumerate() {
            if ui.button(f).clicked() {
                selected_idx = Some(i);
            }
        }
    });
    selected_idx
}

pub fn pick<'a, I>(
    ui: &mut Ui,
    mut folder_iter: I,
    min_width: f32,
    response: &Response,
) -> Option<&'a str>
where
    I: Iterator<Item = &'a str> + Clone,
{
    let popup_id = ui.make_persistent_id("ssh-folder-popup");
    let idx = show_list_popup(ui, folder_iter.clone(), popup_id, min_width, response);

    idx.and_then(|idx| folder_iter.nth(idx))
}
