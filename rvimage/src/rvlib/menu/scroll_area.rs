use egui::{Align, OutputCommand, Pos2, Rect, RichText, Ui};

use crate::paths_selector::PathsSelector;

#[derive(Clone, Copy)]
pub struct ShowFileOptions {
    pub idx: bool,
    pub parentfolder: bool,
}

impl ShowFileOptions {
    pub fn make_file_label(&self, idx: usize, subfolder: &str, file_label: &str) -> String {
        if self.idx && self.parentfolder {
            format!("{idx}) {subfolder}/{file_label}")
        } else if self.idx {
            format!("{idx}) {file_label}")
        } else if self.parentfolder {
            format!("{subfolder}/{file_label}")
        } else {
            file_label.into()
        }
    }
}

impl Default for ShowFileOptions {
    fn default() -> Self {
        Self {
            idx: true,
            parentfolder: false,
        }
    }
}

pub fn scroll_area_file_selector(
    ui: &mut Ui,
    selected_filtered_label_idx: &mut Option<usize>,
    paths_selector: &PathsSelector,
    file_info_selected: Option<&str>,
    scroll_to_selected_label: bool,
    scroll_offset: f32,
    show_file_options: ShowFileOptions,
) -> f32 {
    let scroll_height = ui.available_height() - 200.0;
    let n_rows = paths_selector.len_filtered();
    let text_style = egui::TextStyle::Monospace;
    let row_height = ui.text_style_height(&text_style);
    let spacing_y = ui.spacing().item_spacing.y;
    let area_offset = ui.cursor();

    let target_y = selected_filtered_label_idx
        .map(|idx| area_offset.top() + idx as f32 * (row_height + spacing_y));
    let target_rect = target_y.map(|y| Rect {
        min: Pos2 {
            x: 0.0,
            y: y - scroll_offset,
        },
        max: Pos2 {
            x: 10.0,
            y: y + row_height - scroll_offset,
        },
    });
    let mut add_content = |ui: &mut Ui, filtered_label_idx: usize| {
        let (idx, subfolder, file_label) =
            paths_selector.filtered_idx_file_label_pairs(filtered_label_idx);
        let file_label =
            RichText::new(show_file_options.make_file_label(idx, subfolder, file_label))
                .monospace();
        let sl = if *selected_filtered_label_idx == Some(filtered_label_idx) {
            let path = paths_selector.file_selected_path(filtered_label_idx);
            if let Some(path) = path {
                let sl_ = ui.selectable_label(true, file_label);
                let sl_ = if let Some(fis) = file_info_selected {
                    sl_.on_hover_text(
                        RichText::new(format!("{}\n{fis}", path.path_absolute())).monospace(),
                    )
                } else {
                    sl_.on_hover_text(path.path_absolute())
                };
                if scroll_to_selected_label {
                    sl_.scroll_to_me(Some(Align::Center));
                }
                sl_
            } else {
                ui.selectable_label(false, file_label)
            }
        } else {
            ui.selectable_label(false, file_label)
        };
        if sl.clicked_by(egui::PointerButton::Secondary) {
            // copy to clipboard
            if let Some(fsp) = paths_selector.file_selected_path(filtered_label_idx) {
                ui.output_mut(|po| {
                    po.commands
                        .push(OutputCommand::CopyText(fsp.path_absolute().to_string()));
                });
            }
        }
        if sl.clicked() {
            *selected_filtered_label_idx = Some(filtered_label_idx);
        }
    };
    if scroll_to_selected_label && let Some(tr) = target_rect {
        ui.scroll_to_rect(tr, Some(Align::Center));
    }
    let scroll = egui::ScrollArea::vertical()
        .max_height(scroll_height)
        .show_rows(ui, row_height, n_rows, |ui, row_range| {
            for filtered_idx in row_range {
                add_content(ui, filtered_idx);
            }
        });
    scroll.state.offset.y
}
