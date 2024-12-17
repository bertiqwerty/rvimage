use std::{collections::HashSet, fs, iter, path::Path};

use chrono::{DateTime, Local};
use egui::{Area, Frame, Id, Order, Response, Ui, Widget};
use rvimage_domain::{to_rv, RvResult};

use crate::{
    autosave::{list_files, make_timespan, AUTOSAVE_KEEP_N_DAYS},
    control::Control,
    file_util,
    result::trace_ok_err,
    tools::{BBOX_NAME, BRUSH_NAME},
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

macro_rules! tdm_instance_annos {
    ($name:expr, $func:ident, $func_mut:ident, $tdm:expr, $ui:expr, $cpp_parent:expr, $max_n_folders:expr) => {
        let brush_annos = trace_ok_err($tdm[$name].specifics.$func());
        let mut num_annos = 0;
        let mut parents = vec![];
        if let Some(brush_annos) = brush_annos {
            let parents_set = brush_annos
                .annotations_map
                .iter()
                .flat_map(|(k, (annos, _))| {
                    num_annos += annos.len();
                    Path::new(k).parent().map(Path::to_path_buf)
                })
                .collect::<HashSet<_>>();
            parents = parents_set.into_iter().collect::<Vec<_>>();
            parents.sort();
        }
        let brush_annos_mut = trace_ok_err($tdm.get_mut($name).unwrap().specifics.$func_mut());

        if let Some(brush_annos_mut) = brush_annos_mut {
            $ui.label(" ");
            $ui.label(format!(
                "There are {num_annos} {}-annotations{}.",
                $name,
                if num_annos > 0 {
                    " of images in the following folders"
                } else {
                    ""
                }
            ));
            $ui.end_row();
            for p in &parents[0..$max_n_folders] {
                let p_label = egui::RichText::new(
                    p.to_str()
                        .map(|p| if p.is_empty() { $cpp_parent } else { p })
                        .unwrap_or($cpp_parent),
                )
                .monospace();
                if $ui
                    .button("x")
                    .on_hover_text("double-click to delete all annotations in this folder")
                    .double_clicked()
                {
                    let to_del = brush_annos_mut
                        .annotations_map
                        .keys()
                        .filter(|k| Path::new(k).parent() == Some(&p))
                        .map(|k| k.to_string())
                        .collect::<Vec<_>>();
                    for k in to_del {
                        brush_annos_mut.annotations_map.remove(&k);
                    }
                }
                $ui.label(p_label);

                $ui.end_row();
            }
            if parents.len() > $max_n_folders {
                $ui.label(" ");
                $ui.label(egui::RichText::new("...").monospace());
                $ui.end_row();
            }
        }
    };
}

fn annotations(ui: &mut Ui, tdm: &mut ToolsDataMap, cur_prj_path: &Path) {
    ui.heading("Annotations");
    ui.label(egui::RichText::new(
        "Your project's content is shown below.",
    ));

    let cpp_parent = cur_prj_path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or(".");
    let max_n_folders = 5;
    egui::Grid::new("annotations-menu-grid").show(ui, |ui| {
        tdm_instance_annos!(
            BRUSH_NAME,
            brush,
            brush_mut,
            tdm,
            ui,
            cpp_parent,
            max_n_folders
        );
        tdm_instance_annos!(
            BBOX_NAME,
            bbox,
            bbox_mut,
            tdm,
            ui,
            cpp_parent,
            max_n_folders
        );
    });
}

fn autosaves(ui: &mut Ui, ctrl: &mut Control) -> (Close, Option<ToolsDataMap>) {
    let mut tdm = None;
    let mut close = Close::No;
    let (today, date_n_days_ago) = make_timespan(AUTOSAVE_KEEP_N_DAYS);
    let folder = trace_ok_err(ctrl.cfg.home_folder());
    let folder = folder.map(Path::new);
    let files = trace_ok_err(list_files(folder, Some(date_n_days_ago), Some(today)));
    ui.heading("Reset Annotations to Autsave");
    egui::Grid::new("autosaves-menu-grid").show(ui, |ui| {
        ui.label(egui::RichText::new("name").monospace());
        ui.label(egui::RichText::new("size").monospace());
        ui.label(egui::RichText::new("modified").monospace());
        ui.end_row();
        if let Some(autosaves) = files {
            let cur_prj_path = ctrl.cfg.current_prj_path().to_path_buf();
            let stem = trace_ok_err(file_util::to_stem_str(&cur_prj_path))
                .unwrap_or("default")
                .to_string();
            let files = iter::once(cur_prj_path).chain(autosaves.into_iter().filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with(&stem))
                    == Some(true)
            }));
            let fileinfos = files.clone().map(|path| fileinfo(&path));

            let mut combined: Vec<_> = files
                .zip(fileinfos)
                .flat_map(|(file, info)| info.map(|i| (file, i)))
                .collect();
            combined.sort_by(|(_, (_, datetime1)), (_, (_, datetime2))| datetime1.cmp(datetime2));

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
    (close, tdm)
}

fn annotations_popup(
    ui: &mut Ui,
    ctrl: &mut Control,
    in_tdm: &mut ToolsDataMap,
) -> (Close, Option<ToolsDataMap>) {
    let mut close = Close::No;
    let mut tdm = None;
    Frame::popup(ui.style()).show(ui, |ui| {
        (close, tdm) = autosaves(ui, ctrl);
        ui.separator();
        annotations(ui, in_tdm, ctrl.cfg.current_prj_path());
        ui.separator();
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
        let autosaves_btn_resp = ui.button("Annotations");
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
                    let (close_, tdm) = annotations_popup(ui, self.ctrl, self.tdm);
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
