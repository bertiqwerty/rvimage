use crate::{
    cfg::{self, Cfg},
    control::{Control, Info, SortType},
    file_util::get_prj_name,
    menu::{
        self, cfg_menu::CfgMenu, label_delpropstats::labels_and_sorting, open_folder,
        ui_util::text_edit_singleline,
    },
    tools::ToolState,
    tools_data::ToolSpecifics,
    util::version_label,
    world::ToolsDataMap,
};
use egui::{Area, Context, Frame, Id, Order, Response, RichText, Ui, Widget};
use rvimage_domain::RvResult;
use std::{
    mem,
    path::{Path, PathBuf},
};

use super::{
    label_delpropstats::Stats,
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
    egui::popup_above_or_below_widget(ui, popup_id, response, egui::AboveOrBelow::Above, |ui| {
        let max_msg_len = 500;
        let shortened_msg = if msg.len() > max_msg_len {
            &msg[..max_msg_len]
        } else {
            msg
        };
        ui.label(format!("{icon} {shortened_msg}"));
        new_msg = if ui.button("Close").clicked() {
            Info::None
        } else {
            info_message
        }
    });
    new_msg
}

pub(super) fn get_cfg() -> (Cfg, Info) {
    match cfg::read_cfg() {
        Ok(cfg) => (cfg, Info::None),
        Err(e) => (cfg::get_default_cfg(), Info::Error(format!("{e:?}"))),
    }
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

#[derive(Default)]
struct PopupBtnResp {
    pub resp: Option<Response>,
    pub popup_open: bool,
}

struct Help<'a> {
    id: Id,
    show_help: &'a mut bool,
    export_logs: &'a mut Option<PathBuf>,
}
impl<'a> Help<'a> {
    pub fn new(id: Id, show_help: &'a mut bool, export_logs: &'a mut Option<PathBuf>) -> Self {
        Self {
            id,
            show_help,
            export_logs,
        }
    }
}
impl<'a> Widget for Help<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let help_btn = ui.button("Help");
        if help_btn.clicked() {
            *self.show_help = true;
        }
        if *self.show_help {
            ui.memory_mut(|m| m.open_popup(self.id));
            if ui.memory(|m| m.is_popup_open(self.id)) {
                let area = Area::new(self.id)
                    .order(Order::Foreground)
                    .default_pos(help_btn.rect.left_bottom());
                area.show(ui.ctx(), |ui| {
                    Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label("RV Image\n");
                        const CODE: &str = env!("CARGO_PKG_REPOSITORY");
                        let version_label = version_label();
                        ui.label(version_label);
                        ui.hyperlink_to("Docs, License, and Code", CODE);
                        if ui.button("Export Logs").clicked() {
                            let log_export_dst = rfd::FileDialog::new()
                                .add_filter("zip", &["zip"])
                                .set_file_name("logs.zip")
                                .save_file();

                            *self.export_logs = log_export_dst;
                            ui.memory_mut(|m| m.close_popup());
                            *self.show_help = false;
                        }
                        let resp_close = ui.button("Close");
                        if resp_close.clicked() {
                            ui.memory_mut(|m| m.close_popup());
                            *self.show_help = false;
                        }
                    });
                });
            }
        }
        help_btn
    }
}

fn dialog_in_prjfolder(prj_path: &Path, dialog: rfd::FileDialog) -> rfd::FileDialog {
    if let Some(folder) = prj_path.parent() {
        dialog.set_directory(folder)
    } else {
        dialog
    }
}

pub struct TextBuffers {
    pub filter_string: String,
    pub label_propagation_buffer: String,
    pub label_deletion_buffer: String,
}

pub struct Menu {
    window_open: bool, // Only show the egui window when true.
    info_message: Info,
    are_tools_active: bool,
    scroll_offset: f32,
    open_folder_popup_open: bool,
    load_button_resp: PopupBtnResp,
    stats: Stats,
    filename_sort_type: SortType,
    show_about: bool,
    text_buffers: TextBuffers,
    show_file_idx: bool,
}

impl Menu {
    fn new() -> Self {
        let text_buffers = TextBuffers {
            filter_string: "".to_string(),
            label_propagation_buffer: "".to_string(),
            label_deletion_buffer: "".to_string(),
        };
        Self {
            window_open: true,
            info_message: Info::None,
            are_tools_active: true,
            scroll_offset: 0.0,
            open_folder_popup_open: false,
            load_button_resp: PopupBtnResp::default(),
            stats: Stats::default(),
            filename_sort_type: SortType::default(),
            show_about: false,
            text_buffers,
            show_file_idx: true,
        }
    }
    pub fn sort_type(&self) -> SortType {
        self.filename_sort_type
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
        if let Err(e) = ctrl.load_opened_folder_content(self.filename_sort_type) {
            self.info_message = Info::Error(format!("{e:?}"));
        }
    }

    pub fn show_info(&mut self, msg: Info) {
        self.info_message = msg;
    }

    /// Returns true if a project was loaded
    pub fn ui(
        &mut self,
        ctx: &Context,
        ctrl: &mut Control,
        tools_data_map: &mut ToolsDataMap,
        active_tool_name: Option<&str>,
    ) -> bool {
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

                self.load_button_resp.resp = Some(ui.button("Load Project"));

                let filename =
                    get_prj_name(ctrl.cfg.current_prj_path(), ctrl.opened_folder_label());

                if ui.button("Save Project").clicked() {
                    let prj_path =
                        dialog_in_prjfolder(ctrl.cfg.current_prj_path(), rfd::FileDialog::new())
                            .add_filter("project files", &["json", "rvi"])
                            .set_file_name(filename)
                            .save_file();

                    if let Some(prj_path) = prj_path {
                        handle_error!(ctrl.save(prj_path, tools_data_map, true), self);
                    }
                }
                let popup_id = ui.make_persistent_id("cfg-popup");
                let cfg_gui = CfgMenu::new(popup_id, &mut ctrl.cfg);
                ui.add(cfg_gui);
                let about_popup_id = ui.make_persistent_id("about-popup");
                ui.add(Help::new(
                    about_popup_id,
                    &mut self.show_about,
                    &mut ctrl.log_export_path,
                ));
            });
        });
        let mut projected_loaded = false;
        egui::SidePanel::left("left-main-menu").show(ctx, |ui| {
            if let Some(load_btn_resp) = &self.load_button_resp.resp {
                if load_btn_resp.clicked() {
                    self.load_button_resp.popup_open = true;
                }
                if self.load_button_resp.popup_open {
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
                    self.load_button_resp.resp = None;
                    self.load_button_resp.popup_open = false;
                }
            }
            let mut connected = false;
            handle_error!(
                |con| {
                    connected = con;
                },
                ctrl.check_if_connected(self.filename_sort_type),
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
            handle_error!(
                labels_and_sorting(
                    ui,
                    &mut self.filename_sort_type,
                    ctrl,
                    tools_data_map,
                    &mut self.text_buffers,
                    active_tool_name,
                    &mut self.are_tools_active,
                    &mut self.stats,
                ),
                self
            );
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
