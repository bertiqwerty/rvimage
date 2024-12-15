use std::path::Path;

use egui::{Area, Frame, Id, Order, Response, Ui, Widget};

use crate::{
    autosave::{list_files, make_timespan, AUTOSAVE_KEEP_N_DAYS},
    control::Control,
    result::trace_ok_err,
};

enum Close {
    Yes,
    No,
}

fn autosave_popup(ui: &mut Ui, ctrl: &mut Control) -> Close {
    let mut close = Close::No;

    Frame::popup(ui.style()).show(ui, |ui| {
        let (today, date_n_days_ago) = make_timespan(AUTOSAVE_KEEP_N_DAYS);
        let folder = trace_ok_err(ctrl.cfg.home_folder());
        let folder = folder.map(Path::new);
        let files = trace_ok_err(list_files(folder, Some(date_n_days_ago), Some(today)));
        if let Some(files) = files {
            for p in files {
                if let Some(p_) = p.to_str() {
                    if ui
                        .button(p_)
                        .on_hover_text("double click to switch, loss of unsaved data")
                        .double_clicked()
                    {
                        trace_ok_err(ctrl.replace_with_autosave(p.clone()));
                    }
                }
            }
        }
        ui.horizontal(|ui| {
            if ui.button("Close").clicked() {
                close = Close::Yes;
            }
        })
    });
    close
}

pub struct AutosaveMenu<'a> {
    id: Id,
    ctrl: &'a mut Control,
}
impl<'a> AutosaveMenu<'a> {
    pub fn new(id: Id, ctrl: &'a mut Control) -> AutosaveMenu<'a> {
        Self { id, ctrl }
    }
}
impl Widget for AutosaveMenu<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let autosaves_btn_resp = ui.button("Show Autosaves");
        if autosaves_btn_resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(self.id));
        }
        if ui.memory(|m| m.is_popup_open(self.id)) {
            let area = Area::new(self.id)
                .order(Order::Foreground)
                .default_pos(autosaves_btn_resp.rect.left_bottom());

            let mut close = Close::No;
            let area_response = area
                .show(ui.ctx(), |ui| {
                    close = autosave_popup(ui, self.ctrl);
                })
                .response;
            if let Close::Yes = close {
                ui.memory_mut(|m| m.toggle_popup(self.id));
            }
            if !autosaves_btn_resp.clicked() && area_response.clicked_elsewhere() {
                ui.memory_mut(|m| m.toggle_popup(self.id));
            }
        }
        autosaves_btn_resp
    }
}
