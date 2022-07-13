use egui::{Align, Pos2, Rect, Ui};

use crate::paths_selector::PathsSelector;

pub fn scroll_area(
    ui: &mut Ui,
    file_selected_idx: &mut Option<usize>,
    paths_selector: &PathsSelector,
    scroll_to_selected_label: bool,
    scroll_offset: f32,
) -> f32 {
    let scroll_height = ui.available_height() - 120.0;
    let n_rows = paths_selector.file_labels().len();
    let text_style = egui::TextStyle::Body;
    let row_height = ui.text_style_height(&text_style);
    let spacing_y = ui.spacing().item_spacing.y;
    let area_offset = ui.cursor();

    let target_y =
        file_selected_idx.map(|idx| area_offset.top() + idx as f32 * (row_height + spacing_y));
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
    let mut add_content = |ui: &mut Ui, filtered_idx: usize| {
        let file_label = paths_selector.file_labels()[filtered_idx].1.as_str();
        let sl = if *file_selected_idx == Some(filtered_idx) {
            let path = paths_selector.file_selected_path(filtered_idx);
            let sl_ = ui.selectable_label(true, file_label).on_hover_text(path);
            if scroll_to_selected_label {
                sl_.scroll_to_me(Some(Align::Center));
            }
            sl_
        } else {
            ui.selectable_label(false, file_label)
        };
        if sl.clicked() {
            *file_selected_idx = Some(filtered_idx);
        }
    };
    if scroll_to_selected_label {
        if let Some(tr) = target_rect {
            ui.scroll_to_rect(tr, Some(Align::Center));
        }
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
