use egui::{Align, Ui};

use crate::{
    reader::{LoadImageForGui, ReaderFromCfg},
    result::RvResult,
};

pub fn scroll_area(
    ui: &mut Ui,
    file_selected_idx: &mut Option<usize>,
    file_labels: &Vec<(usize, String)>,
    scroll_to_selected_label: &mut bool,
    reader: &mut ReaderFromCfg,
) -> RvResult<()> {
    for (gui_idx, (reader_idx, s)) in file_labels.iter().enumerate() {
        let sl = if *file_selected_idx == Some(gui_idx) {
            let path = reader.file_selected_path()?;
            let sl_ = ui.selectable_label(true, s).on_hover_text(path);
            if *scroll_to_selected_label {
                sl_.scroll_to_me(Align::Center);
            }
            sl_
        } else {
            ui.selectable_label(false, s)
        };
        if sl.clicked() {
            println!("ri {} / gi {}", reader_idx, gui_idx);
            reader.select_file(*reader_idx);
            *file_selected_idx = Some(gui_idx);
        }
    }
    *scroll_to_selected_label = false;
    Ok(())
}
