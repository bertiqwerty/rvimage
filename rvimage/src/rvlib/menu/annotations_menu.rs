use std::{collections::HashSet, fs, iter, path::Path};

use chrono::{DateTime, Local};
use egui::{Area, Frame, Id, Order, Response, Ui, Widget};
use rvimage_domain::{to_rv, RvResult};

use crate::{
    autosave::{list_files, make_timespan, AUTOSAVE_KEEP_N_DAYS},
    control::Control,
    file_util::{self, PathPair},
    result::trace_ok_err,
    tools::{BBOX_NAME, BRUSH_NAME},
    tools_data::{AnnotationsMap, ToolSpecifics},
    world::ToolsDataMap,
    InstanceAnnotate,
};

use super::ui_util::slider;

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

struct FolderParams {
    max_n_folders: usize,
    parents_depth: u8,
}

fn ancestor(path: &String, depth: u8) -> &Path {
    Path::new(path)
        .ancestors()
        .nth(depth.into())
        .unwrap_or(Path::new(""))
}

fn get_absent_files<'a, T>(
    tool_name: &str,
    unwrap_specifics: impl Fn(&ToolSpecifics) -> RvResult<&AnnotationsMap<T>>,
    tdm: &'a ToolsDataMap,
    filepaths: &[&PathPair],
) -> RvResult<Vec<&'a str>>
where
    T: InstanceAnnotate + 'a,
{
    if tdm.contains_key(tool_name) {
        let datamap = unwrap_specifics(&tdm[tool_name].specifics)?;
        Ok(datamap
            .keys()
            .filter(|k| !filepaths.iter().any(|fp| fp.path_relative() == *k))
            .map(String::as_str)
            .collect::<Vec<_>>())
    } else {
        Ok(vec![])
    }
}

fn get_all_absent_files<'a>(
    tdm: &'a ToolsDataMap,
    filepaths: &[&PathPair],
) -> Vec<(&'a str, &'static str)> {
    let mut all_absent_files = vec![];
    let afs = trace_ok_err(get_absent_files(
        BBOX_NAME,
        |ts| ts.bbox().map(|d| &d.annotations_map),
        tdm,
        filepaths,
    ));
    if let Some(afs) = afs {
        all_absent_files.extend(afs.into_iter().map(|af| (af, BBOX_NAME)));
    }
    let afs = trace_ok_err(get_absent_files(
        BRUSH_NAME,
        |ts| ts.brush().map(|d| &d.annotations_map),
        tdm,
        filepaths,
    ));
    if let Some(afs) = afs {
        all_absent_files.extend(afs.into_iter().map(|af| (af, BRUSH_NAME)));
    }
    all_absent_files
}

fn tdm_instance_annos<T>(
    name: &str,
    tdm: &mut ToolsDataMap,
    ui: &mut Ui,
    folder_params: FolderParams,
    unwrap_specifics: impl Fn(&ToolSpecifics) -> RvResult<&AnnotationsMap<T>>,
    unwrap_specifics_mut: impl Fn(&mut ToolSpecifics) -> RvResult<&mut AnnotationsMap<T>>,
) where
    T: InstanceAnnotate,
{
    let FolderParams {
        max_n_folders,
        parents_depth,
    } = folder_params;
    if tdm.contains_key(name) {
        let anno_map = trace_ok_err(unwrap_specifics(&tdm[name].specifics));
        let mut n_annos_allfolders = 0;
        let mut parents = vec![];
        if let Some(brush_annos) = anno_map {
            let parents_set = brush_annos
                .iter()
                .map(|(k, (annos, _))| {
                    n_annos_allfolders += annos.len();
                    ancestor(k, parents_depth).to_path_buf()
                })
                .collect::<HashSet<_>>();
            parents = parents_set.into_iter().collect::<Vec<_>>();
            parents.sort();
        }
        let anno_map_mut = trace_ok_err(unwrap_specifics_mut(
            &mut tdm.get_mut(name).unwrap().specifics,
        ));

        if let Some(annos_map_mut) = anno_map_mut {
            ui.label(format!(
                "There are {n_annos_allfolders} {}-annotations{}.",
                name,
                if n_annos_allfolders > 0 {
                    " of images in the following folders"
                } else {
                    ""
                }
            ));
            egui::Grid::new("annotations-menu-grid").show(ui, |ui| {
                for p in &parents[0..max_n_folders.min(parents.len())] {
                    let p_label = egui::RichText::new(p.to_str().unwrap_or("")).monospace();
                    let n_annos_of_subfolders = egui::RichText::new(format!(
                        "{}",
                        annos_map_mut
                            .iter()
                            .filter(|(k, _)| ancestor(k, parents_depth) == p)
                            .map(|(_, (anno_map, _))| anno_map.len())
                            .sum::<usize>()
                    ))
                    .monospace();
                    if ui
                        .button("x")
                        .on_hover_text("double-click to delete all annotations in this folder")
                        .double_clicked()
                    {
                        let to_del = annos_map_mut
                            .keys()
                            .filter(|k| ancestor(k, parents_depth) == p)
                            .map(|k| k.to_string())
                            .collect::<Vec<_>>();
                        for k in to_del {
                            annos_map_mut.remove(&k);
                        }
                    }
                    ui.label(n_annos_of_subfolders);
                    ui.label(p_label);

                    ui.end_row();
                }
                if parents.len() > max_n_folders {
                    ui.label(" ");
                    ui.label(egui::RichText::new("...").monospace());
                    ui.end_row();
                }
            });
        }
    }
}

fn annotations<'a>(
    ui: &mut Ui,
    tdm: &mut ToolsDataMap,
    are_tools_active: &mut bool,
    parents_depth: &mut u8,
    get_filtered_filespaths: impl Fn() -> Option<Vec<&'a PathPair>>,
) -> RvResult<()> {
    ui.heading("Annotations");
    ui.label(egui::RichText::new(
        "Your project's content is shown below.",
    ));

    slider(
        ui,
        are_tools_active,
        parents_depth,
        1..=5u8,
        "# subfolders to aggregate",
    );
    let max_n_folders = 5;
    egui::ScrollArea::vertical().show(ui, |ui| {
        tdm_instance_annos(
            BRUSH_NAME,
            tdm,
            ui,
            FolderParams {
                max_n_folders,
                parents_depth: *parents_depth,
            },
            |ts| ts.brush().map(|d| &d.annotations_map),
            |ts| ts.brush_mut().map(|d| &mut d.annotations_map),
        );
        tdm_instance_annos(
            BBOX_NAME,
            tdm,
            ui,
            FolderParams {
                max_n_folders,
                parents_depth: *parents_depth,
            },
            |ts| ts.bbox().map(|d| &d.annotations_map),
            |ts| ts.bbox_mut().map(|d| &mut d.annotations_map),
        );
    });
    if ui
        .button("Log annotated files not in the filelist")
        .clicked()
    {
        let filepaths = get_filtered_filespaths();
        if let Some(filepaths) = filepaths {
            let absent_files = get_all_absent_files(tdm, &filepaths);
            if absent_files.is_empty() {
                tracing::info!("no absent files with annotations found");
            }
            for (af, tool_name) in absent_files {
                tracing::info!("absent file {af} has {tool_name} annotations ");
            }
        }
    }
    if ui
        .button("Delete annotations of files not in the filelist")
        .clicked()
    {
        let filepaths = get_filtered_filespaths();
        if let Some(filepaths) = filepaths {
            let absent_files = get_all_absent_files(tdm, &filepaths);
            let absent_files = absent_files
                .into_iter()
                .map(|(af, tn)| (af.to_string(), tn))
                .collect::<Vec<_>>();
            for (af, tool_name) in absent_files {
                tracing::info!("deleting annotations of {af} for tool {tool_name}");
                if tool_name == BBOX_NAME {
                    let tools_data = tdm.get_mut(tool_name);
                    if let Some(td) = tools_data {
                        td.specifics.bbox_mut()?.annotations_map.remove(&af);
                    }
                }
                if tool_name == BRUSH_NAME {
                    let tools_data = tdm.get_mut(tool_name);
                    if let Some(td) = tools_data {
                        td.specifics.brush_mut()?.annotations_map.remove(&af);
                    }
                }
            }
        }
    }
    Ok(())
}

fn autosaves(ui: &mut Ui, ctrl: &mut Control) -> (Close, Option<ToolsDataMap>) {
    let mut tdm = None;
    let mut close = Close::No;
    let (today, date_n_days_ago) = make_timespan(AUTOSAVE_KEEP_N_DAYS);
    let folder = Path::new(ctrl.cfg.home_folder());
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
    are_tools_active: &mut bool,
    parent_depth: &mut u8,
) -> (Close, Option<ToolsDataMap>) {
    let mut close = Close::No;
    let mut tdm = None;
    Frame::popup(ui.style()).show(ui, |ui| {
        (close, tdm) = autosaves(ui, ctrl);
        ui.separator();
        let get_filelist = || {
            let filelist = ctrl
                .paths_navigator
                .paths_selector()
                .map(|ps| ps.filtered_file_paths());
            filelist
        };
        trace_ok_err(annotations(
            ui,
            in_tdm,
            are_tools_active,
            parent_depth,
            get_filelist,
        ));
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
    are_tools_active: &'a mut bool,
    parents_depth: &'a mut u8,
}
impl<'a> AutosaveMenu<'a> {
    pub fn new(
        id: Id,
        ctrl: &'a mut Control,
        tools_data_map: &'a mut ToolsDataMap,
        project_loaded: &'a mut bool,
        are_tools_active: &'a mut bool,
        parents_depth: &'a mut u8,
    ) -> AutosaveMenu<'a> {
        Self {
            id,
            ctrl,
            tdm: tools_data_map,
            project_loaded,
            are_tools_active,
            parents_depth,
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
                    let (close_, tdm) = annotations_popup(
                        ui,
                        self.ctrl,
                        self.tdm,
                        self.are_tools_active,
                        self.parents_depth,
                    );
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
