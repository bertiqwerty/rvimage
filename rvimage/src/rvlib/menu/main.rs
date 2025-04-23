use crate::{
    cfg::ExportPathConnection,
    control::{Control, Info},
    file_util::{get_prj_name, path_to_str},
    image_reader::LoadImageForGui,
    menu::{
        self,
        annotations_menu::{AnnotationsParams, AutosaveMenu},
        cfg_menu::CfgMenu,
        file_counts::labels_and_sorting,
        open_folder,
        ui_util::text_edit_singleline,
    },
    tools::ToolState,
    tools_data::{ToolSpecifics, ToolsDataMap},
    util::version_label,
};
use egui::{Context, Id, Response, RichText, Ui};
use rvimage_domain::RvResult;
use std::{
    mem,
    path::{Path, PathBuf},
};

use super::{
    file_counts::Counts,
    tools_menus::{attributes_menu, bbox_menu, brush_menu},
};

fn show_popup(
    ui: &mut Ui,
    msg: &str,
    icon: &str,
    popup_id: Id,
    info_message: Info,
    response: &Response,
) -> Info {
    ui.memory_mut(|m| m.open_popup(popup_id));
    let mut new_msg = Info::None;
    egui::popup_above_or_below_widget(
        ui,
        popup_id,
        response,
        egui::AboveOrBelow::Above,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
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
        },
    );
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
                ToolSpecifics::Attributes(x) => {
                    attributes_menu(ui, v.menu_active, mem::take(x), &mut self.are_tools_active)
                }
                _ => Ok(mem::take(v)),
            };
            *v = tmp?;
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
}

pub struct Menu {
    window_open: bool, // Only show the egui window when true.
    info_message: Info,
    are_tools_active: bool,
    toggle_clear_cache_on_close: bool,
    scroll_offset: f32,
    open_folder_popup_open: bool,
    stats: Counts,
    text_buffers: TextBuffers,
    show_file_idx: bool,
    annotations_menu_params: AnnotationsParams,
    import_coco_from_ssh: bool,
    new_file_idx_annoplot: Option<usize>,
}

impl Menu {
    fn new() -> Self {
        let text_buffers = TextBuffers {
            filter_string: "".to_string(),
            label_propagation: "".to_string(),
            label_deletion: "".to_string(),
            import_coco_from_ssh_path: "path on ssh server".to_string(),
        };
        Self {
            window_open: true,
            info_message: Info::None,
            are_tools_active: true,
            toggle_clear_cache_on_close: false,
            scroll_offset: 0.0,
            open_folder_popup_open: false,
            stats: Counts::default(),
            text_buffers,
            show_file_idx: true,
            annotations_menu_params: AnnotationsParams::default(),
            import_coco_from_ssh: false,
            new_file_idx_annoplot: None,
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
        ctx: &Context,
        ctrl: &mut Control,
        tools_data_map: &mut ToolsDataMap,
        active_tool_name: Option<&str>,
    ) -> bool {
        let mut projected_loaded = false;
        egui::TopBottomPanel::top("top-menu-bar").show(ctx, |ui| {
            // Top row with open folder and settings button
            egui::menu::bar(ui, |ui| {
                let button_resp = open_folder::button(ui, ctrl, self.open_folder_popup_open);
                handle_error!(
                    |open| {
                        self.open_folder_popup_open = open;
                    },
                    || self.open_folder_popup_open = false,
                    button_resp,
                    self
                );
                ui.menu_button("Project", |ui| {
                    if ui
                        .button("New")
                        .on_hover_text(
                            "Double click, old project will be closed, unsaved data will get lost",
                        )
                        .double_clicked()
                    {
                        *tools_data_map = ctrl.new_prj();
                        ui.close_menu();
                    }
                    if ui.button("Load").clicked() {
                        let prj_path = rfd::FileDialog::new()
                            .add_filter("project files", &["json", "rvi"])
                            .pick_file();
                        if let Some(prj_path) = prj_path {
                            handle_error!(
                                |tdm| {
                                    *tools_data_map = tdm;
                                    projected_loaded = true;
                                },
                                ctrl.load(prj_path),
                                self
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save").clicked() {
                        let prj_path = save_dialog_in_prjfolder(
                            ctrl.cfg.current_prj_path(),
                            ctrl.opened_folder_label(),
                        );

                        if let Some(prj_path) = prj_path {
                            handle_error!(ctrl.save(prj_path, tools_data_map, true), self);
                        }
                        ui.close_menu();
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
                                    projected_loaded = true;
                                },
                                ctrl.import_annos(&prj_path, tools_data_map),
                                self
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("... Settings").clicked() {
                        let prj_path = rfd::FileDialog::new()
                            .set_title("Import Settings from Project")
                            .add_filter("project files", &["json", "rvi"])
                            .pick_file();
                        if let Some(prj_path) = prj_path {
                            handle_error!(
                                |()| {
                                    projected_loaded = true;
                                },
                                ctrl.import_settings(&prj_path),
                                self
                            );
                        }
                        ui.close_menu();
                    }
                    if ui.button("... Annotations and Settings").clicked() {
                        let prj_path = rfd::FileDialog::new()
                            .set_title("Import Annotations and Settings from Project")
                            .add_filter("project files", &["json", "rvi"])
                            .pick_file();
                        if let Some(prj_path) = prj_path {
                            handle_error!(
                                |()| {
                                    projected_loaded = true;
                                },
                                ctrl.import_both(&prj_path, tools_data_map),
                                self
                            );
                        }
                        ui.close_menu();
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
                                        projected_loaded = true;
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
                            ui.close_menu();
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

                let popup_id = ui.make_persistent_id("autosave-popup");
                let autosave_gui = AutosaveMenu::new(
                    popup_id,
                    ctrl,
                    tools_data_map,
                    &mut projected_loaded,
                    &mut self.are_tools_active,
                    &mut self.annotations_menu_params,
                    &mut self.new_file_idx_annoplot,
                );
                ui.add(autosave_gui);
                ctrl.paths_navigator
                    .select_label_idx(self.new_file_idx_annoplot);

                let popup_id = ui.make_persistent_id("cfg-popup");
                let cfg_gui = CfgMenu::new(
                    popup_id,
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
                        ui.close_menu();
                    }
                    let resp_close = ui.button("Close");
                    if resp_close.clicked() {
                        ui.close_menu();
                    }
                });
            });
        });
        egui::SidePanel::left("left-main-menu").show(ctx, |ui| {
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
            let popup_id = ui.make_persistent_id("info-popup");
            self.info_message = match &self.info_message {
                Info::Warning(msg) => show_popup(
                    ui,
                    msg,
                    "❕",
                    popup_id,
                    self.info_message.clone(),
                    &filter_txt_field,
                ),
                Info::Error(msg) => show_popup(
                    ui,
                    msg,
                    "❌",
                    popup_id,
                    self.info_message.clone(),
                    &filter_txt_field,
                ),
                Info::None => Info::None,
            };

            // scroll area showing image file names
            let scroll_to_selected = ctrl.paths_navigator.scroll_to_selected_label();
            let mut filtered_label_selected_idx = ctrl.paths_navigator.file_label_selected_idx();
            if let Some(ps) = &ctrl.paths_navigator.paths_selector() {
                ui.checkbox(&mut self.show_file_idx, "show file index");

                self.scroll_offset = menu::scroll_area::scroll_area_file_selector(
                    ui,
                    &mut filtered_label_selected_idx,
                    ps,
                    ctrl.file_info_selected.as_deref(),
                    scroll_to_selected,
                    self.scroll_offset,
                    self.show_file_idx,
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
        });
        projected_loaded
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
