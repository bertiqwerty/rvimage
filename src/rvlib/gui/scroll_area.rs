use egui::{Align, Ui};

use crate::paths_selector::PathsSelector;

pub fn scroll_area(
    ui: &mut Ui,
    file_selected_idx: &mut Option<usize>,
    paths_selector: &PathsSelector,
    scroll_to_selected_label: bool,
) {
    optick::event!();
    let scroll_height = ui.available_height() - 120.0;
    let n_rows = paths_selector.file_labels().len();
    egui::ScrollArea::vertical()
        .max_height(scroll_height)
        .show(ui, |ui| {
            for filtered_idx in 0..n_rows {
                let file_label = paths_selector.file_labels()[filtered_idx].1.as_str();
                let sl = if *file_selected_idx == Some(filtered_idx) {
                    let path = paths_selector.file_selected_path(filtered_idx);
                    let sl_ = ui.selectable_label(true, file_label).on_hover_text(path);
                    if scroll_to_selected_label {
                        sl_.scroll_to_me(Align::Center);
                    }
                    sl_
                } else {
                    ui.selectable_label(false, file_label)
                };
                if sl.clicked() {
                    *file_selected_idx = Some(filtered_idx);
                }
            }
        });
}
