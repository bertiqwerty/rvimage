use egui::{Align, Ui};

use crate::{paths_selector::PathsSelector, result::RvResult};

pub fn scroll_area(
    ui: &mut Ui,
    file_selected_idx: &mut Option<usize>,
    paths_selector: &PathsSelector,
    scroll_to_selected_label: bool,
) -> RvResult<()> {
    optick::event!();
    for (gui_idx, (_, s)) in paths_selector.file_labels().iter().enumerate() {
        let sl = if *file_selected_idx == Some(gui_idx) {
            let path = paths_selector.file_selected_path(gui_idx);
            let sl_ = ui.selectable_label(true, s).on_hover_text(path);
            if scroll_to_selected_label {
                sl_.scroll_to_me(Align::Center);
            }
            sl_
        } else {
            ui.selectable_label(false, s)
        };
        if sl.clicked() {
            *file_selected_idx = Some(gui_idx);
        }
    }
    Ok(())
}
