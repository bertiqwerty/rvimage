use crate::{
    cfg::{self, Cfg},
    control::{Control, Info, SortType},
    domain::Annotate,
    file_util::get_prj_name,
    menu::{self, cfg_menu::CfgMenu, open_folder, ui_util::text_edit_singleline},
    paths_selector::PathsSelector,
    result::RvResult,
    tools::ToolState,
    tools_data::{AnnotationsMap, ToolSpecifics},
    util::version_label,
    world::ToolsDataMap,
};
use egui::{Area, Context, Frame, Id, Order, Response, RichText, Ui, Widget};
use std::{mem, path::PathBuf};

use super::tools_menus::{attributes_menu, bbox_menu, brush_menu};

pub fn n_annotated_images<T>(annotations_map: &AnnotationsMap<T>, paths: &[&str]) -> usize
where
    T: Annotate,
{
    paths
        .iter()
        .filter(|p| {
            if let Some((anno, _)) = annotations_map.get(**p) {
                !anno.elts().is_empty()
            } else {
                false
            }
        })
        .count()
}
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
                ToolSpecifics::Bbox(x) => {
                    bbox_menu(ui, v.menu_active, mem::take(x), &mut self.are_tools_active)
                }
                ToolSpecifics::Brush(x) => {
                    brush_menu(ui, v.menu_active, mem::take(x), &mut self.are_tools_active)
                }
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

#[derive(Default)]
struct Stats {
    n_files_filtered_info: Option<String>,
    n_files_annotated_info: Option<String>,
}

#[derive(Default)]
struct ImportPrjState {
    show: bool,
    is_import_triggered: bool,
    old_path: String,
    new_path: String,
}
struct ImportPrj<'a> {
    id: Id,
    state: &'a mut ImportPrjState,
    are_tools_active: &'a mut bool,
}
impl<'a> ImportPrj<'a> {
    pub fn new(id: Id, state: &'a mut ImportPrjState, are_tools_active: &'a mut bool) -> Self {
        Self {
            id,
            state,
            are_tools_active,
        }
    }
}
impl<'a> Widget for ImportPrj<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let import_show_poup_btn = ui.button("Import Project");
        if import_show_poup_btn.clicked() {
            self.state.show = true;
        }
        if self.state.show {
            ui.memory_mut(|m| m.open_popup(self.id));
            if ui.memory(|m| m.is_popup_open(self.id)) {
                let area = Area::new(self.id)
                    .order(Order::Foreground)
                    .default_pos(import_show_poup_btn.rect.left_bottom());
                area.show(ui.ctx(), |ui| {
                    Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label("Map base path from");
                        text_edit_singleline(ui, &mut self.state.old_path, self.are_tools_active);
                        ui.label("to");
                        ui.horizontal(|ui| {
                            text_edit_singleline(
                                ui,
                                &mut self.state.new_path,
                                self.are_tools_active,
                            );
                            if ui.button("Select").clicked() {
                                let src_path = rfd::FileDialog::new().pick_folder();
                                if let Some(src_path) =
                                    src_path.and_then(|p| p.to_str().map(|s| s.to_string()))
                                {
                                    self.state.new_path = src_path;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.button("Import").clicked() {
                                self.state.is_import_triggered = true;
                            }
                            let resp_close = ui.button("Close");
                            if resp_close.clicked() || self.state.is_import_triggered {
                                ui.memory_mut(|m| m.close_popup());
                                self.state.show = false;
                            }
                        });
                    });
                });
            }
        }
        import_show_poup_btn
    }
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

pub struct Menu {
    window_open: bool, // Only show the egui window when true.
    info_message: Info,
    filter_string: String,
    are_tools_active: bool,
    editable_ssh_cfg_str: String,
    scroll_offset: f32,
    open_folder_popup_open: bool,
    load_button_resp: PopupBtnResp,
    stats: Stats,
    filename_sort_type: SortType,
    show_about: bool,
    import_prj_state: ImportPrjState,
}

impl Menu {
    fn new() -> Self {
        let (cfg, _) = get_cfg();
        let ssh_cfg_str = toml::to_string_pretty(&cfg.ssh_cfg).unwrap();
        Self {
            window_open: true,
            info_message: Info::None,
            filter_string: "".to_string(),
            are_tools_active: true,
            editable_ssh_cfg_str: ssh_cfg_str,
            scroll_offset: 0.0,
            open_folder_popup_open: false,
            load_button_resp: PopupBtnResp::default(),
            stats: Stats::default(),
            filename_sort_type: SortType::default(),
            show_about: false,
            import_prj_state: ImportPrjState::default(),
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
                    let prj_path = rfd::FileDialog::new()
                        .add_filter("project files", &["rvi"])
                        .set_file_name(filename)
                        .save_file();

                    if let Some(prj_path) = prj_path {
                        handle_error!(ctrl.save(prj_path, tools_data_map, true), self);
                    }
                }
                let import_prj_id = ui.make_persistent_id("import-prj-popup");
                ui.add(ImportPrj::new(
                    import_prj_id,
                    &mut self.import_prj_state,
                    &mut self.are_tools_active,
                ));

                let popup_id = ui.make_persistent_id("cfg-popup");
                let cfg_gui = CfgMenu::new(
                    popup_id,
                    &mut ctrl.cfg,
                    &mut self.editable_ssh_cfg_str,
                    &mut self.are_tools_active,
                );
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
                if self.import_prj_state.is_import_triggered {
                    self.import_prj_state.is_import_triggered = false;
                    let import_prj_path = rfd::FileDialog::new()
                        .add_filter("project files", &["json", "rvi"])
                        .pick_file();
                    if let Some(import_prj_path) = import_prj_path {
                        handle_error!(
                            |tdm| {
                                *tools_data_map = tdm;
                                projected_loaded = true;
                            },
                            ctrl.import(
                                import_prj_path,
                                self.import_prj_state.old_path.as_str(),
                                self.import_prj_state.new_path.as_str()
                            ),
                            self
                        );
                    }
                }
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

            let filter_txt_field =
                text_edit_singleline(ui, &mut self.filter_string, &mut self.are_tools_active);

            if filter_txt_field.changed() {
                handle_error!(
                    ctrl.paths_navigator.filter(
                        &self.filter_string,
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
                self.scroll_offset = menu::scroll_area::scroll_area(
                    ui,
                    &mut filtered_label_selected_idx,
                    ps,
                    ctrl.file_info_selected.as_deref(),
                    scroll_to_selected,
                    self.scroll_offset,
                );
                ctrl.paths_navigator.deactivate_scroll_to_selected_label();
                if ctrl.paths_navigator.file_label_selected_idx() != filtered_label_selected_idx {
                    ctrl.paths_navigator
                        .select_label_idx(filtered_label_selected_idx);
                }
            }

            ui.separator();
            let clicked_nat = ui
                .radio_value(
                    &mut self.filename_sort_type,
                    SortType::Natural,
                    "Natural Sorting",
                )
                .clicked();
            let clicked_alp = ui
                .radio_value(
                    &mut self.filename_sort_type,
                    SortType::Alphabetical,
                    "Alphabetical Sorting",
                )
                .clicked();
            if clicked_nat || clicked_alp {
                handle_error!(
                    |_| {},
                    ctrl.sort(
                        self.filename_sort_type,
                        &self.filter_string,
                        tools_data_map,
                        active_tool_name
                    ),
                    self
                );
                handle_error!(|_| {}, ctrl.reload(self.filename_sort_type), self);
            }
            if let Some(info) = &self.stats.n_files_filtered_info {
                ui.label(info);
            }
            if let Some(info) = &self.stats.n_files_annotated_info {
                ui.label(info);
            }
            let get_file_info = |ps: &PathsSelector| {
                let n_files_filtered = ps.len_filtered();
                Some(format!("{n_files_filtered} files"))
            };
            let get_annotation_info = |ps: &PathsSelector| {
                if let Some(active_tool_name) = active_tool_name {
                    if let Some(data) = tools_data_map.get(active_tool_name) {
                        let paths = &ps.filtered_file_paths();
                        let n = data.specifics.apply(
                            |d| Ok(n_annotated_images(&d.annotations_map, paths)),
                            |d| Ok(n_annotated_images(&d.annotations_map, paths)),
                        );
                        n.ok()
                            .map(|n| format!("{n} files with {active_tool_name} annotations"))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            if let Some(ps) = ctrl.paths_navigator.paths_selector() {
                if self.stats.n_files_filtered_info.is_none() {
                    self.stats.n_files_filtered_info = get_file_info(ps);
                }
                if self.stats.n_files_annotated_info.is_none() {
                    self.stats.n_files_annotated_info = get_annotation_info(ps);
                }
                if ui.button("Re-compute Stats").clicked() {
                    self.stats.n_files_filtered_info = get_file_info(ps);
                    self.stats.n_files_annotated_info = get_annotation_info(ps);
                }
            } else {
                self.stats.n_files_filtered_info = None;
                self.stats.n_files_annotated_info = None;
            }
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
