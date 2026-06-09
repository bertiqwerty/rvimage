use crate::{
    cfg::{ExportPathConnection, WandPrjMessage},
    control::{Control, Info, PrjSettingImportSection},
    file_util::{get_prj_name, path_to_str},
    image_reader::LoadImageForGui,
    menu::{
        self,
        annotations_menu::{AnnotationsParams, AutosaveMenu},
        cfg_menu::CfgMenu,
        file_counts::labels_and_sorting,
        open_folder,
        scroll_area::ShowFileOptions,
        ui_util::{removable_rows, slider, text_edit_multiline, text_edit_singleline},
    },
    tools::ToolState,
    tools_data::{ToolSpecifics, ToolsDataMap},
    util::version_label,
};
use core::f32;
use egui::{Popup, Response, RichText, Ui};
use rvimage_domain::{RvResult, rverr};
use std::{
    mem,
    path::{Path, PathBuf},
};

use super::{
    file_counts::Counts,
    tools_menus::{attributes_menu, bbox_menu, brush_menu},
};

fn show_popup(msg: &str, icon: &str, info_message: Info, response: &Response) -> Info {
    let mut new_msg = Info::None;
    Popup::from_response(response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            let max_msg_len = 500;
            let shortened_msg = if msg.len() > max_msg_len {
                &msg[..max_msg_len]
            } else {
                msg
            };
            let mut txt = format!("{icon} {shortened_msg}");
            ui.text_edit_multiline(&mut txt);
            new_msg = if ui.button("Close").clicked() {
                Info::None
            } else {
                info_message
            }
        });
    new_msg
}

// evaluates an expression that is expected to return Result,
// passes unpacked value to effect function in case of Ok,
// sets according error message in case of Err.
// Closure $f_err_cleanup will be called in case of an error.
macro_rules! handle_error {
    ($f_effect:expr, $f_err_cleanup:expr, $result:expr, $self:expr) => {
        match $result {
            Ok(r) => {
                #[allow(clippy::redundant_closure_call)]
                $f_effect(r);
            }
            Err(e) => {
                #[allow(clippy::redundant_closure_call)]
                $f_err_cleanup();
                tracing::error!("{e:?}");
                $self.info_message = Info::Error(e.to_string());
            }
        }
    };
    ($effect:expr, $result:expr, $self:expr) => {
        handle_error!($effect, || (), $result, $self)
    };
    ($result:expr, $self:expr) => {
        handle_error!(|_| {}, $result, $self);
    };
}

pub struct ToolSelectMenu {
    are_tools_active: bool, // can deactivate all tools, overrides activated_tool
    recently_activated_tool: Option<usize>,
}
impl ToolSelectMenu {
    fn new() -> Self {
        Self {
            are_tools_active: true,
            recently_activated_tool: None,
        }
    }
    pub fn recently_clicked_tool(&self) -> Option<usize> {
        self.recently_activated_tool
    }
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        tools: &mut [ToolState],
        tools_menu_map: &mut ToolsDataMap,
    ) -> RvResult<()> {
        ui.horizontal_top(|ui| {
            self.recently_activated_tool = tools
                .iter_mut()
                .enumerate()
                .filter(|(_, t)| !t.is_always_active())
                .find(|(_, t)| ui.selectable_label(t.is_active(), t.button_label).clicked())
                .map(|(i, _)| i);
        });
        for v in tools_menu_map.values_mut().filter(|v| v.menu_active) {
            let mut v_result = Err(rverr!("Tool menu not implemented"));
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(f32::INFINITY)
                .show(ui, |ui| {
                    let tmp = match &mut v.specifics {
                        ToolSpecifics::Bbox(x) => bbox_menu(
                            ui,
                            v.menu_active,
                            mem::take(x),
                            &mut self.are_tools_active,
                            v.visible_inactive_tools.clone(),
                        ),
                        ToolSpecifics::Brush(x) => brush_menu(
                            ui,
                            v.menu_active,
                            mem::take(x),
                            &mut self.are_tools_active,
                            v.visible_inactive_tools.clone(),
                        ),
                        ToolSpecifics::Attributes(x) => attributes_menu(
                            ui,
                            v.menu_active,
                            mem::take(x),
                            &mut self.are_tools_active,
                        ),
                        _ => Ok(mem::take(v)),
                    };
                    v_result = tmp;
                });
            *v = v_result?;
        }
        Ok(())
    }
}
impl Default for ToolSelectMenu {
    fn default() -> Self {
        Self::new()
    }
}

fn save_dialog_in_prjfolder(prj_path: &Path, opened_folder: Option<&str>) -> Option<PathBuf> {
    let filename = get_prj_name(prj_path, opened_folder);
    let dialog = rfd::FileDialog::new();
    let dialog = if let Some(folder) = prj_path.parent() {
        dialog.set_directory(folder)
    } else {
        dialog
    };
    dialog
        .add_filter("project files", &["json", "rvi"])
        .set_file_name(filename)
        .save_file()
}

#[derive(Default)]
pub struct TextBuffers {
    pub filter_string: String,
    pub label_propagation: String,
    pub label_deletion: String,
    pub import_coco_from_ssh_path: String,
    pub wand_prj_annotator_comment: String,
    pub wand_prj_annotator_exclfolder: String,
}

pub struct Menu {
    window_open: bool, // Only show the egui window when true.
    info_message: Info,
    are_tools_active: bool,
    toggle_clear_cache_on_close: bool,
    scroll_offset: f32,
    stats: Counts,
    text_buffers: TextBuffers,
    show_file_options: ShowFileOptions,
    annotations_menu_params: AnnotationsParams,
    import_coco_from_ssh: bool,
    new_file_idx_annoplot: Option<usize>,
    prj_import_path: Option<PathBuf>,
    prj_import_section: PrjSettingImportSection,
    prj_settings_for_display: Option<String>,
    cache_all_progress: Option<f32>,
    show_wandprjannotator: bool,
}

impl Menu {
    fn new() -> Self {
        let text_buffers = TextBuffers {
            filter_string: "".into(),
            label_propagation: "".into(),
            label_deletion: "".into(),
            import_coco_from_ssh_path: "path on ssh server".into(),
            wand_prj_annotator_comment: "".into(),
            wand_prj_annotator_exclfolder: "".into(),
        };
        Self {
            window_open: true,
            info_message: Info::None,
            are_tools_active: true,
            toggle_clear_cache_on_close: false,
            scroll_offset: 0.0,
            stats: Counts::default(),
            text_buffers,
            show_file_options: ShowFileOptions::default(),
            annotations_menu_params: AnnotationsParams::default(),
            import_coco_from_ssh: false,
            new_file_idx_annoplot: None,
            prj_import_path: None,
            prj_import_section: PrjSettingImportSection::All,
            prj_settings_for_display: None,
            cache_all_progress: None,
            show_wandprjannotator: false,
        }
    }
    pub fn popup(&mut self, info: Info) {
        self.info_message = info;
    }

    pub fn toggle(&mut self) {
        if self.window_open {
            self.are_tools_active = true;
            self.window_open = false;
        } else {
            self.window_open = true;
        }
    }

    pub fn reload_opened_folder(&mut self, ctrl: &mut Control) {
        if let Err(e) = ctrl.load_opened_folder_content(ctrl.cfg.prj.sort_params) {
            self.info_message = Info::Error(format!("{e:?}"));
        }
    }

    pub fn show_info(&mut self, msg: Info) {
        self.info_message = msg;
    }

    /// Returns true if a project was loaded and if a new file load was triggered
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        ctrl: &mut Control,
        tools_data_map: &mut ToolsDataMap,
        active_tool_name: Option<&str>,
    ) -> bool {
        let mut project_loaded = false;
        egui::Panel::top("top-menu-panel").show_inside(ui, |ui| {
            // Top row with open folder and settings button
            egui::MenuBar::new().ui(ui, |ui| {
                let of_response = ui.button("Open Folder");
                let pick_result = open_folder::pick_by_connection(ctrl, &of_response);
                handle_error!(pick_result, self);
                ui.menu_button("Project", |ui| {
                    if ui
                        .button("New")
                        .on_hover_text(
                            "Right click, old project will be closed, unsaved data will get lost",
                        )
                        .secondary_clicked()
                    {
                        *tools_data_map = ctrl.new_prj();
                        ui.close();
                    }
                    if ui.button("Load").clicked() {
                        let prj_path = rfd::FileDialog::new()
                            .add_filter("project files", &["json", "rvi"])
                            .pick_file();
                        if let Some(prj_path) = prj_path {
                            handle_error!(
                                |tdm| {
                                    *tools_data_map = tdm;
                                    project_loaded = true;
                                },
                                ctrl.load(prj_path),
                                self
                            );
                        }
                        ui.close();
                    }
                    if ui.button("Save").clicked() {
                        let prj_path = save_dialog_in_prjfolder(
                            ctrl.cfg.current_prj_path(),
                            ctrl.opened_folder_label(),
                        );

                        if let Some(prj_path) = prj_path {
                            handle_error!(ctrl.save(prj_path, tools_data_map, true), self);
                        }
                        ui.close();
                    }
                    ui.separator();
                    ui.label("Import ...");
                    if ui.button("... Annotations").clicked() {
                        let prj_path = rfd::FileDialog::new()
                            .set_title("Import Annotations from Project")
                            .add_filter("project files", &["json", "rvi"])
                            .pick_file();
                        if let Some(prj_path) = prj_path {
                            handle_error!(
                                |()| {
                                    project_loaded = true;
                                },
                                ctrl.import_annos(&prj_path, tools_data_map),
                                self
                            );
                        }
                        ui.close();
                    }

                    if ui.button("... Settings").clicked() {
                        // First pick a project file, then open the modal to confirm import options.
                        if let Some(prj_path) = rfd::FileDialog::new()
                            .set_title("Pick Project to Import Settings From")
                            .add_filter("project files", &["json", "rvi"])
                            .pick_file()
                        {
                            self.prj_import_path = Some(prj_path);
                        }
                        ui.close();
                    }
                    if ui.button("... Annotations and Settings").clicked() {
                        let prj_path = rfd::FileDialog::new()
                            .set_title("Import Annotations and Settings from Project")
                            .add_filter("project files", &["json", "rvi"])
                            .pick_file();
                        if let Some(prj_path) = prj_path {
                            handle_error!(
                                |()| {
                                    project_loaded = true;
                                },
                                ctrl.import_both(&prj_path, tools_data_map),
                                self
                            );
                        }
                        ui.close();
                    }
                    ui.horizontal(|ui| {
                        if ui.button("... Annotations from COCO file").clicked() {
                            let coco_path = if !self.import_coco_from_ssh {
                                rfd::FileDialog::new()
                                    .set_title("Annotations from COCO file")
                                    .add_filter("coco files", &["json"])
                                    .pick_file()
                                    .and_then(|p| path_to_str(&p).ok().map(|s| s.to_string()))
                            } else {
                                Some(self.text_buffers.import_coco_from_ssh_path.clone())
                            };
                            if let Some(coco_path) = coco_path {
                                handle_error!(
                                    |()| {
                                        project_loaded = true;
                                    },
                                    ctrl.import_from_coco(
                                        &coco_path,
                                        tools_data_map,
                                        if self.import_coco_from_ssh {
                                            ExportPathConnection::Ssh
                                        } else {
                                            ExportPathConnection::Local
                                        }
                                    ),
                                    self
                                );
                            }
                            ui.close();
                        }
                        ui.checkbox(&mut self.import_coco_from_ssh, "ssh")
                    });

                    if self.import_coco_from_ssh {
                        text_edit_singleline(
                            ui,
                            &mut self.text_buffers.import_coco_from_ssh_path,
                            &mut self.are_tools_active,
                        );
                    }
                });

                let autosave_gui = AutosaveMenu::new(
                    ctrl,
                    tools_data_map,
                    &mut project_loaded,
                    &mut self.are_tools_active,
                    &mut self.annotations_menu_params,
                    &mut self.new_file_idx_annoplot,
                );
                ui.add(autosave_gui);
                ctrl.paths_navigator
                    .select_label_idx(self.new_file_idx_annoplot);

                let cfg_gui = CfgMenu::new(
                    &mut ctrl.cfg,
                    &mut self.are_tools_active,
                    &mut self.toggle_clear_cache_on_close,
                );
                ui.add(cfg_gui);
                if self.toggle_clear_cache_on_close {
                    if let Some(reader) = &mut ctrl.reader {
                        reader.toggle_clear_cache_on_close();
                    }
                    self.toggle_clear_cache_on_close = false;
                }

                ui.menu_button("Wand", |ui| {
                    if ui.button("Predict").clicked() {
                        self.show_wandprjannotator = true;
                        // handle_error!(ctrl.ask_wand_for_prj_annotations(), self);
                    }
                    ui.separator();
                    if ui.button("Start Wand Server").clicked() {
                        handle_error!(ctrl.start_wandserver(), self);
                    }
                    if ui.button("Cleanup Wand Server").clicked() {
                        handle_error!(ctrl.cleanup_wandserver(), self);
                    }
                });
                if self.show_wandprjannotator {
                    let mut assess_tmp = ctrl
                        .cfg
                        .prj
                        .wand_prj_annotator
                        .messages
                        .iter()
                        .last()
                        .and_then(|msg| msg.success_assessment);
                    egui::modal::Modal::new(egui::Id::new("prj-import-section")).show(
                        ui.ctx(),
                        |ui| {
                            ui.heading("Wand to annotate all filtered project images");
                            let wpa = &ctrl.cfg.prj.wand_prj_annotator;
                            let len_msgs = wpa.messages.len();
                            let mut idx_to_remove = None;
                            ui.separator();
                            egui::ScrollArea::vertical()
                                .max_height(300.0)
                                .show(ui, |ui| {
                                    idx_to_remove = removable_rows(ui, len_msgs, |ui, idx| {
                                        let mut job = egui::text::LayoutJob {
                                            halign: egui::Align::RIGHT,
                                            ..Default::default()
                                        };
                                        job.append(
                                            &wpa.messages[idx].comment,
                                            0.0,
                                            egui::TextFormat {
                                                italics: true,
                                                ..Default::default()
                                            },
                                        );
                                        ui.label(job);

                                        if idx < len_msgs.saturating_sub(1) {
                                            ui.label(
                                                &wpa.messages[idx]
                                                    .success_assessment
                                                    .map(|a| format!("assessment {a}"))
                                                    .unwrap_or("".to_string()),
                                            );
                                        } else if let Some(response) = &wpa.messages[idx].response {
                                            egui::CollapsingHeader::new("Response")
                                                .id_salt(idx)
                                                .show(ui, |ui| {
                                                    ui.label(response);
                                                });
                                            let mut assess_checkbx = assess_tmp.is_some();
                                            if ui
                                                .checkbox(&mut assess_checkbx, "assess result")
                                                .clicked()
                                            {
                                                if assess_checkbx {
                                                    assess_tmp = Some(50u8);
                                                } else {
                                                    assess_tmp = None;
                                                }
                                            }
                                            if let Some(assess) = assess_tmp.as_mut() {
                                                slider(
                                                    ui,
                                                    &mut self.are_tools_active,
                                                    assess,
                                                    0..=100,
                                                    "assess result",
                                                );
                                            }
                                        }

                                        ui.separator();
                                    });
                                });
                            if let Some(idx) = idx_to_remove {
                                ctrl.cfg.prj.wand_prj_annotator.messages.remove(idx);
                            }
                            if let Some(assess_last) =
                                ctrl.cfg.prj.wand_prj_annotator.messages.iter_mut().last()
                            {
                                assess_last.success_assessment = assess_tmp;
                            }

                            text_edit_multiline(
                                ui,
                                &mut self.text_buffers.wand_prj_annotator_comment,
                                &mut self.are_tools_active,
                            );

                            ui.horizontal(|ui| {
                                if ui.button("Add comment").clicked()
                                    && !self
                                        .text_buffers
                                        .wand_prj_annotator_comment
                                        .trim()
                                        .is_empty()
                                {
                                    ctrl.cfg.prj.wand_prj_annotator.messages.push(
                                        WandPrjMessage::from_comment(mem::take(
                                            &mut self.text_buffers.wand_prj_annotator_comment,
                                        )),
                                    );
                                }
                                if ui.button("Clear").clicked() {
                                    ctrl.cfg.prj.wand_prj_annotator.messages.clear();
                                }
                            });
                            ui.separator();
                            text_edit_singleline(
                                ui,
                                &mut self.text_buffers.wand_prj_annotator_exclfolder,
                                &mut self.are_tools_active,
                            );
                            if ui.button("Add folder to exclude").clicked()
                                && !self
                                    .text_buffers
                                    .wand_prj_annotator_exclfolder
                                    .trim()
                                    .is_empty()
                            {
                                ctrl.cfg.prj.wand_prj_annotator.subfolder_to_exclude.push(
                                    mem::take(&mut self.text_buffers.wand_prj_annotator_exclfolder),
                                )
                            }

                            let n_folders =
                                ctrl.cfg.prj.wand_prj_annotator.subfolder_to_exclude.len();
                            ui.separator();
                            if n_folders > 0 {
                                ui.label("Folders to exclude");
                                let mut idx_remove = None;
                                egui::Grid::new("label_grid").num_columns(2).show(ui, |ui| {
                                    idx_remove = removable_rows(ui, n_folders, |ui, idx| {
                                        ui.label(
                                            &ctrl.cfg.prj.wand_prj_annotator.subfolder_to_exclude
                                                [idx],
                                        );
                                        ui.end_row();
                                    });
                                });
                                if let Some(idx) = idx_remove {
                                    ctrl.cfg
                                        .prj
                                        .wand_prj_annotator
                                        .subfolder_to_exclude
                                        .remove(idx);
                                }
                                ui.separator();
                            }
                            if ui.button("Submit").clicked() {
                                self.show_wandprjannotator = false;
                                let files = ctrl.paths_navigator.paths_selector().map(|ps| {
                                    ps.filtered_abs_file_paths()
                                        .iter()
                                        .map(|p| p.to_string())
                                        .collect::<Vec<String>>()
                                });
                                if let Some(files) = files {
                                    let folders_to_exclude = ctrl
                                        .cfg
                                        .prj
                                        .wand_prj_annotator
                                        .subfolder_to_exclude
                                        .clone();
                                    ctrl.submit_prj_to_wandannotator(
                                        tools_data_map,
                                        &files,
                                        &folders_to_exclude,
                                    );
                                } else {
                                    tracing::warn!("No files selected to submit to wand annotator");
                                }
                            }
                            ui.separator();
                            if ui.button("Close").clicked() {
                                self.show_wandprjannotator = false;
                            }
                        },
                    );
                }
                ui.menu_button("Help", |ui| {
                    ui.label("RV Image\n");
                    const CODE: &str = env!("CARGO_PKG_REPOSITORY");
                    let version_label = version_label();
                    ui.label(version_label);
                    if let Some(reader) = &mut ctrl.reader {
                        ui.label("cache size in mb");
                        ui.label(
                            egui::RichText::new(format!("{:.3}", reader.cache_size_in_mb()))
                                .monospace(),
                        );
                        ui.label("Hit F5 to clear the cache.");
                        ui.label("");
                    }
                    ui.hyperlink_to("Docs, License, and Code", CODE);
                    if ui.button("Export Logs").clicked() {
                        let log_export_dst = rfd::FileDialog::new()
                            .add_filter("zip", &["zip"])
                            .set_file_name("logs.zip")
                            .save_file();

                        ctrl.log_export_path = log_export_dst;
                        ui.close();
                    }
                    let resp_close = ui.button("Close");
                    if resp_close.clicked() {
                        ui.close();
                    }
                });
            });
        });
        // Show project settings import modal when a file was picked.
        if self.prj_import_path.is_some() {
            egui::modal::Modal::new(egui::Id::new("prj-import-section")).show(ui.ctx(), |ui| {
                ui.label("Project Settings Import");
                if let Some(p) = &self.prj_import_path {
                    ui.label(RichText::new(format!("{}", p.display())).monospace());
                }
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        if ui
                            .radio_value(
                                &mut self.prj_import_section,
                                PrjSettingImportSection::All,
                                "All",
                            )
                            .clicked()
                        {
                            changed = true;
                        }
                        if ui
                            .radio_value(
                                &mut self.prj_import_section,
                                PrjSettingImportSection::Connection,
                                "Connection",
                            )
                            .clicked()
                        {
                            changed = true;
                        }
                        if ui
                            .radio_value(
                                &mut self.prj_import_section,
                                PrjSettingImportSection::WandServer,
                                "Wand Server",
                            )
                            .clicked()
                        {
                            changed = true;
                        }
                    });
                    if let Some(p) = &self.prj_import_path {
                        if self.prj_settings_for_display.is_none() || changed {
                            self.prj_settings_for_display =
                                Some(ctrl.show_settings(p, self.prj_import_section));
                        }
                        if let Some(settings_str) = &mut self.prj_settings_for_display {
                            egui::ScrollArea::vertical()
                                .min_scrolled_height(500.0)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(settings_str)
                                            .font(egui::FontSelection::Style(
                                                egui::TextStyle::Monospace,
                                            ))
                                            .desired_width(f32::INFINITY)
                                            .desired_rows(20) // control height
                                            .interactive(false), // make it non-editable
                                    );
                                });
                        }
                    }
                });
                ui.horizontal(|ui| {
                    let import_enabled = self.prj_import_path.is_some();
                    if import_enabled
                        && ui.button("Import").clicked()
                        && let Some(prj_path) = self.prj_import_path.take()
                    {
                        handle_error!(
                            |()| {
                                project_loaded = true;
                            },
                            ctrl.import_settings(&prj_path, self.prj_import_section),
                            self
                        );
                    }
                    if ui.button("Cancel").clicked() {
                        self.prj_import_path = None;
                    }
                });
            });
        }
        egui::Panel::left("left-main-menu").show_inside(ui, |ui| {
            let mut connected = false;
            handle_error!(
                |con| {
                    connected = con;
                },
                ctrl.check_if_connected(ctrl.cfg.prj.sort_params),
                self
            );
            if connected {
                ui.label(
                    RichText::from(ctrl.opened_folder_label().unwrap_or(""))
                        .text_style(egui::TextStyle::Monospace),
                );
            } else {
                ui.label(RichText::from("Connecting...").text_style(egui::TextStyle::Monospace));
            }

            let filter_txt_field = text_edit_singleline(
                ui,
                &mut self.text_buffers.filter_string,
                &mut self.are_tools_active,
            );

            if filter_txt_field.changed() {
                handle_error!(
                    ctrl.paths_navigator.filter(
                        &self.text_buffers.filter_string,
                        tools_data_map,
                        active_tool_name
                    ),
                    self
                );
            }
            // Popup for error messages
            self.info_message = match &self.info_message {
                Info::Warning(msg) => {
                    show_popup(msg, "❕", self.info_message.clone(), &filter_txt_field)
                }
                Info::Error(msg) => {
                    show_popup(msg, "❌", self.info_message.clone(), &filter_txt_field)
                }
                Info::None => Info::None,
            };

            // scroll area showing image file names
            let scroll_to_selected = ctrl.paths_navigator.scroll_to_selected_label();
            let mut filtered_label_selected_idx = ctrl.paths_navigator.file_label_selected_idx();
            if let Some(ps) = &ctrl.paths_navigator.paths_selector() {
                ui.checkbox(&mut self.show_file_options.idx, "show file index");
                ui.checkbox(
                    &mut self.show_file_options.parentfolder,
                    "show parent folder",
                );

                self.scroll_offset = menu::scroll_area::scroll_area_file_selector(
                    ui,
                    &mut filtered_label_selected_idx,
                    ps,
                    ctrl.file_info_selected.as_deref(),
                    scroll_to_selected,
                    self.scroll_offset,
                    self.show_file_options,
                );
                ctrl.paths_navigator.deactivate_scroll_to_selected_label();
                if ctrl.paths_navigator.file_label_selected_idx() != filtered_label_selected_idx {
                    ctrl.paths_navigator
                        .select_label_idx(filtered_label_selected_idx);
                }
            }

            ui.separator();
            let mut sort_params = ctrl.cfg.prj.sort_params;
            handle_error!(
                labels_and_sorting(
                    ui,
                    &mut sort_params,
                    ctrl,
                    tools_data_map,
                    &mut self.text_buffers,
                    &mut self.stats,
                ),
                self
            );
            ctrl.cfg.prj.sort_params = sort_params;
            if ui
                .button("Pre-cache filtered images")
                .on_hover_text("double click")
                .double_clicked()
            {
                self.cache_all_progress = Some(0.0);
            }
            if self.cache_all_progress.is_some() {
                handle_error!(
                    |prgs| {
                        self.cache_all_progress = prgs;
                    },
                    ctrl.cache_all_filtered(),
                    self
                );
                if let Some(prgs) = &self.cache_all_progress {
                    ui.add(
                        egui::ProgressBar::new(*prgs).text(
                            RichText::new(format!(
                                "loading images into cache {:2}%",
                                (prgs * 100.0).floor() as u8
                            ))
                            .monospace(),
                        ),
                    );
                }
                if self.cache_all_progress > Some(0.999) {
                    self.cache_all_progress = None;
                }
            }
        });
        project_loaded
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

pub fn are_tools_active(menu: &Menu, tsm: &ToolSelectMenu) -> bool {
    menu.are_tools_active && tsm.are_tools_active
}
