use std::{fs, iter, path::Path};

use chrono::{DateTime, Local};
use egui::{Area, Frame, Id, Order, Response, Ui, Widget};
use rvimage_domain::{to_rv, RvResult};

use crate::{
    autosave::{list_files, make_timespan, AUTOSAVE_KEEP_N_DAYS},
    control::Control,
    result::trace_ok_err,
    world::ToolsDataMap,
};

enum Close {
    Yes,
    No,
}

fn fileinfo(path: &Path) -> RvResult<(String, String)> {
    let metadata = fs::metadata(path).map_err(to_rv)?;
    let n_bytes = metadata.len();
    let mb = n_bytes as f64 / (1024.0f64).powi(2);
    let mb = format!("{mb:0.3}mb");

    let modified = metadata.modified().map_err(to_rv)?;
    let datetime: DateTime<Local> = modified.into();
    let datetime = datetime.format("%b %d %Y - %H:%M:%S").to_string();
    Ok((mb, datetime))
}

fn autosave_popup(ui: &mut Ui, ctrl: &mut Control) -> (Close, Option<ToolsDataMap>) {
    let mut close = Close::No;
    let mut tdm = None;
    Frame::popup(ui.style()).show(ui, |ui| {
        let (today, date_n_days_ago) = make_timespan(AUTOSAVE_KEEP_N_DAYS);
        let folder = trace_ok_err(ctrl.cfg.home_folder());
        let folder = folder.map(Path::new);
        let files = trace_ok_err(list_files(folder, Some(date_n_days_ago), Some(today)));

        egui::Grid::new("autosave-menu-grid").show(ui, |ui| {
            ui.label(egui::RichText::new("name").monospace());
            ui.label(egui::RichText::new("size").monospace());
            ui.label(egui::RichText::new("modified").monospace());
            ui.end_row();
            if let Some(autosaves) = files {
                let cur_prj_path = ctrl.cfg.current_prj_path().to_path_buf();
                let files = iter::once(cur_prj_path).chain(autosaves);
                let fileinfos = files.clone().map(|path| fileinfo(&path));

                let mut combined: Vec<_> = files
                    .zip(fileinfos)
                    .flat_map(|(file, info)| info.map(|i| (file, i)))
                    .collect();
                combined
                    .sort_by(|(_, (_, datetime1)), (_, (_, datetime2))| datetime1.cmp(datetime2));

                for (path, (mb, datetime)) in combined.iter().rev() {
                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                        if ui
                            .button(egui::RichText::new(file_name).monospace())
                            .on_hover_text("double click to apply, LOSS(💀) of unsaved data")
                            .double_clicked()
                        {
                            tdm = trace_ok_err(ctrl.replace_with_save(path));
                            close = Close::Yes;
                        }
                        ui.label(egui::RichText::new(mb).monospace());
                        ui.label(egui::RichText::new(datetime).monospace());
                        ui.end_row();
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Close").clicked() {
                close = Close::Yes;
            }
        })
    });
    (close, tdm)
}

pub struct AutosaveMenu<'a> {
    id: Id,
    ctrl: &'a mut Control,
    tdm: &'a mut ToolsDataMap,
    project_loaded: &'a mut bool,
}
impl<'a> AutosaveMenu<'a> {
    pub fn new(
        id: Id,
        ctrl: &'a mut Control,
        tools_data_map: &'a mut ToolsDataMap,
        project_loaded: &'a mut bool,
    ) -> AutosaveMenu<'a> {
        Self {
            id,
            ctrl,
            tdm: tools_data_map,
            project_loaded,
        }
    }
}
impl Widget for AutosaveMenu<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        *self.project_loaded = false;
        let autosaves_btn_resp = ui.button("Autosaved Annotations");
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
                    let (close_, tdm) = autosave_popup(ui, self.ctrl);
                    close = close_;
                    if let Some(tdm) = tdm {
                        *self.tdm = tdm;
                        *self.project_loaded = true;
                    }
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
