use crate::{
    cfg::{self, Cfg},
    control::{Control, Info},
    file_util::{self, ConnectionData, MetaData},
    menu::{self, cfg_menu::CfgMenu, open_folder},
    result::{to_rv, RvResult},
    tools::ToolState,
    tools_data::ToolSpecifics,
    world::ToolsDataMap,
};
use egui::{ClippedPrimitive, Context, Id, Pos2, Response, TexturesDelta, Ui};
use egui_wgpu::renderer::{RenderPass, ScreenDescriptor};
use image::DynamicImage;
use pixels::{wgpu, PixelsContext};
use std::{mem, path::Path};
use winit::window::Window;

use super::tools_menus::bbox_menu;

/// Manages all state required for rendering egui over `Pixels`.
pub struct Framework {
    // State for egui.
    egui_ctx: Context,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    rpass: RenderPass,
    paint_jobs: Vec<ClippedPrimitive>,
    textures: TexturesDelta,
    // State for the GUI
    menu: Menu,
    tool_selection_menu: ToolSelectMenu,
}

impl Framework {
    /// Create egui.
    pub fn new(width: u32, height: u32, scale_factor: f32, pixels: &pixels::Pixels) -> Self {
        let egui_ctx = Context::default();
        let max_texture_size = pixels.device().limits().max_texture_dimension_2d as usize;
        let egui_state = egui_winit::State::from_pixels_per_point(max_texture_size, scale_factor);

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: scale_factor,
        };
        let rpass = RenderPass::new(pixels.device(), pixels.render_texture_format(), 1);
        let menu = Menu::new();
        let tools_menu = ToolSelectMenu::new();
        let textures = TexturesDelta::default();
        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            rpass,
            paint_jobs: Vec::new(),
            textures,
            menu,
            tool_selection_menu: tools_menu,
        }
    }

    pub fn cfg_of_opened_folder(&self) -> Option<&Cfg> {
        self.menu.control.reader().map(|r| r.cfg())
    }

    pub fn opened_folder(&self) -> Option<&String> {
        self.menu.control.opened_folder.as_ref()
    }

    pub fn meta_data(&self, file_selected: Option<usize>) -> MetaData {
        let file_path =
            file_selected.and_then(|fs| self.menu().file_path(fs).map(|s| s.to_string()));
        let open_folder = self.opened_folder().cloned();
        let ssh_cfg = self.cfg_of_opened_folder().map(|cfg| cfg.ssh_cfg.clone());
        let connection_data = match ssh_cfg {
            Some(ssh_cfg) => ConnectionData::Ssh(ssh_cfg),
            None => ConnectionData::None,
        };
        let export_folder = self
            .cfg_of_opened_folder()
            .map(|cfg| cfg.export_folder().map(|ef| ef.to_string()).unwrap());
        MetaData {
            file_path,
            connection_data,
            opened_folder: open_folder,
            export_folder,
        }
    }
    /// Handle input events from the window manager.
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        self.egui_state.on_event(&self.egui_ctx, event);
    }

    /// Resize egui.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.size_in_pixels = [width, height];
        }
    }

    /// Update scaling factor.
    pub fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.pixels_per_point = scale_factor as f32;
    }

    /// Prepare egui.
    pub fn prepare(
        &mut self,
        window: &Window,
        tools: &mut [ToolState],
        tools_data_map: &mut ToolsDataMap,
    ) {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            // Draw menus.
            self.menu.ui(egui_ctx, tools_data_map);
            self.tool_selection_menu.ui(egui_ctx, tools, tools_data_map);
        });
        self.textures.append(output.textures_delta);
        self.egui_state
            .handle_platform_output(window, &self.egui_ctx, output.platform_output);
        self.paint_jobs = self.egui_ctx.tessellate(output.shapes);
    }

    /// Render egui.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext,
    ) {
        // Upload all resources to the GPU.
        for (id, image_delta) in &self.textures.set {
            self.rpass
                .update_texture(&context.device, &context.queue, *id, image_delta);
        }

        self.rpass.update_buffers(
            &context.device,
            &context.queue,
            &self.paint_jobs,
            &self.screen_descriptor,
        );

        // Record all render passes.
        self.rpass.execute(
            encoder,
            render_target,
            &self.paint_jobs,
            &self.screen_descriptor,
            None,
        );

        // Cleanup
        let textures = std::mem::take(&mut self.textures);
        for id in &textures.free {
            self.rpass.free_texture(id);
        }
    }

    pub fn menu(&self) -> &Menu {
        &self.menu
    }

    pub fn menu_mut(&mut self) -> &mut Menu {
        &mut self.menu
    }

    pub fn are_tools_active(&self) -> bool {
        self.menu.are_tools_active && self.tool_selection_menu.are_tools_active
    }

    pub fn recently_activated_tool(&self) -> Option<usize> {
        self.tool_selection_menu.recently_activated_tool
    }

    pub fn toggle_tools_menu(&mut self) {
        self.tool_selection_menu.toggle();
    }
}

fn show_popup(
    ui: &mut Ui,
    msg: &str,
    icon: &str,
    popup_id: Id,
    info_message: Info,
    below_respone: &Response,
) -> Info {
    ui.memory().open_popup(popup_id);
    let mut new_msg = Info::None;
    egui::popup_below_widget(ui, popup_id, below_respone, |ui| {
        let max_msg_len = 500;
        let shortened_msg = if msg.len() > max_msg_len {
            &msg[..max_msg_len]
        } else {
            msg
        };
        ui.label(format!("{} {}", icon, shortened_msg));
        new_msg = if ui.button("close").clicked() {
            Info::None
        } else {
            info_message
        }
    });
    new_msg
}

pub(super) fn get_cfg() -> (Cfg, Info) {
    match cfg::get_cfg() {
        Ok(cfg) => (cfg, Info::None),
        Err(e) => (cfg::get_default_cfg(), Info::Error(format!("{:?}", e))),
    }
}
// evaluates an expression that is expected to return Result,
// passes unpacked value to effect function in case of Ok,
// sets according error message in case of Err
macro_rules! handle_error {
    ($effect:expr, $result:expr, $self:expr) => {
        match $result {
            Ok(r) => {
                $effect(r);
            }
            Err(e) => {
                $self.info_message = Info::Error(e.to_string());
            }
        }
    };
    ($result:expr, $self:expr) => {
        handle_error!(|_| {}, $result, $self);
    };
}

pub struct ToolSelectMenu {
    window_open: bool,      // Only show the egui window when true.
    are_tools_active: bool, // can deactivate all tools, overrides activated_tool
    recently_activated_tool: Option<usize>,
}
impl ToolSelectMenu {
    fn new() -> Self {
        Self {
            window_open: true,
            are_tools_active: true,
            recently_activated_tool: None,
        }
    }

    fn ui(&mut self, ctx: &Context, tools: &mut [ToolState], tools_menu_map: &mut ToolsDataMap) {
        let window_response = egui::Window::new("tools")
            .vscroll(true)
            .title_bar(false)
            .open(&mut self.window_open)
            .default_pos(Pos2 { x: 500.0, y: 15.0 })
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    self.recently_activated_tool = tools
                        .iter_mut()
                        .enumerate()
                        .find(|(_, t)| ui.selectable_label(t.is_active(), t.button_label).clicked())
                        .map(|(i, _)| i);
                });
                for v in tools_menu_map.values_mut().filter(|v| v.menu_active) {
                    *v = match &mut v.specifics {
                        ToolSpecifics::Bbox(x) => bbox_menu(ui, v.menu_active, mem::take(x)),
                        ToolSpecifics::Brush(_) => mem::take(v),
                    };
                }
            });
        if let (Some(wr), Some(pos)) = (window_response, ctx.pointer_latest_pos()) {
            if wr.response.rect.expand(5.0).contains(pos) {
                self.are_tools_active = false;
            } else {
                self.are_tools_active = true;
            }
        }
    }

    pub fn toggle(&mut self) {
        if self.window_open {
            self.window_open = false;
        } else {
            self.window_open = true;
        }
    }
}

pub struct Menu {
    control: Control,
    window_open: bool, // Only show the egui window when true.
    info_message: Info,
    filter_string: String,
    are_tools_active: bool,
    editable_ssh_cfg_str: String,
    scroll_offset: f32,
    open_folder_popup_open: bool,
    import_triggered: bool,
}

impl Menu {
    fn new() -> Self {
        let (cfg, _) = get_cfg();
        let ssh_cfg_str = toml::to_string_pretty(&cfg.ssh_cfg).unwrap();
        Self {
            control: Control::new(cfg),
            window_open: true,
            info_message: Info::None,
            filter_string: "".to_string(),
            are_tools_active: true,
            editable_ssh_cfg_str: ssh_cfg_str,
            scroll_offset: 0.0,
            open_folder_popup_open: false,
            import_triggered: false,
        }
    }

    pub fn popup(&mut self, info: Info) {
        self.info_message = info;
    }

    pub fn toggle(&mut self) {
        if self.window_open {
            self.window_open = false;
        } else {
            self.window_open = true;
        }
    }

    pub fn prev(&mut self) {
        self.control.paths_navigator.prev();
    }

    pub fn next(&mut self) {
        self.control.paths_navigator.next();
    }

    pub fn file_label_selected_idx(&self) -> Option<usize> {
        self.control.paths_navigator.file_label_selected_idx()
    }

    fn idx_of_file_label(&self, file_label: &str) -> Option<usize> {
        match self.control.paths_navigator.paths_selector() {
            Some(ps) => ps.idx_of_file_label(file_label),
            None => None,
        }
    }

    pub fn reload_opened_folder(&mut self) {
        if let Err(e) = self.control.load_opened_folder_content() {
            self.info_message = Info::Error(format!("{:?}", e));
        }
    }

    pub fn file_label(&self, idx: usize) -> &str {
        match self.control.paths_navigator.paths_selector() {
            Some(ps) => ps.file_labels()[idx].1.as_str(),
            None => "",
        }
    }

    pub fn select_file_label(&mut self, file_label: &str) {
        self.control
            .paths_navigator
            .select_label_idx(self.idx_of_file_label(file_label));
    }

    pub fn activate_scroll_to_label(&mut self) {
        self.control
            .paths_navigator
            .activate_scroll_to_selected_label();
    }

    pub fn select_label_idx(&mut self, file_label_idx: Option<usize>) {
        self.control
            .paths_navigator
            .select_label_idx(file_label_idx);
    }

    pub fn folder_label(&self) -> Option<&str> {
        self.control
            .paths_navigator
            .paths_selector()
            .as_ref()
            .map(|ps| ps.folder_label())
    }

    pub fn file_path(&self, file_idx: usize) -> Option<&str> {
        self.control
            .paths_navigator
            .paths_selector()
            .as_ref()
            .map(|ps| ps.file_selected_path(file_idx))
    }

    pub fn read_image(
        &mut self,
        file_label_selected_idx: usize,
        reload: bool,
    ) -> Option<DynamicImage> {
        let mut im_res = None;
        handle_error!(
            |im| {
                im_res = im;
            },
            self.control.read_image(file_label_selected_idx, reload),
            self
        );
        im_res
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &Context, tools_data_map: &mut ToolsDataMap) {
        let window_response = egui::Window::new("menu")
            .vscroll(true)
            .open(&mut self.window_open)
            .show(ctx, |ui| {
                // Popup for error messages
                let popup_id = ui.make_persistent_id("info-popup");
                let r = ui.separator();
                self.info_message = match &self.info_message {
                    Info::Warning(msg) => {
                        show_popup(ui, msg, "❕", popup_id, self.info_message.clone(), &r)
                    }
                    Info::Error(msg) => {
                        show_popup(ui, msg, "❌", popup_id, self.info_message.clone(), &r)
                    }
                    Info::None => Info::None,
                };

                // Top row with open folder and settings button
                ui.horizontal(|ui| {
                    let button_resp =
                        open_folder::button(ui, &mut self.control, self.open_folder_popup_open);
                    handle_error!(
                        |open| {
                            self.open_folder_popup_open = open;
                        },
                        button_resp,
                        self
                    );
                    let popup_id = ui.make_persistent_id("cfg-popup");

                    if ui.button("import").clicked() {
                        self.import_triggered = true;
                    }

                    let cfg_gui = CfgMenu::new(
                        popup_id,
                        &mut self.control.cfg,
                        &mut self.editable_ssh_cfg_str,
                    );
                    ui.add(cfg_gui);
                });

                if let Ok(folder) = self.control.cfg.export_folder() {
                    if self.import_triggered {
                        let mut filename_for_export = None;
                        let mut exports = || -> RvResult<()> {
                            for filename in file_util::exports_in_folder(folder)
                                .map_err(to_rv)?
                                .filter_map(|p| {
                                    p.file_name().map(|p| p.to_str().map(|p| p.to_string()))
                                })
                                .flatten()
                            {
                                if ui.button(&filename).clicked() {
                                    filename_for_export = Some(filename);
                                }
                            }
                            Ok(())
                        };
                        handle_error!(exports(), self);
                        if let Some(ffe) = filename_for_export {
                            let file_path = Path::new(folder).join(ffe);
                            handle_error!(self.control.import(file_path, tools_data_map), self);
                            self.import_triggered = false;
                        }
                    }
                }
                let mut connected = false;
                handle_error!(
                    |con| {
                        connected = con;
                    },
                    self.control.check_if_connected(),
                    self
                );
                if connected {
                    ui.label(self.control.opened_folder_label().unwrap_or(""));
                } else {
                    ui.label("connecting...");
                }

                // filter text field
                let txt_field = ui.text_edit_singleline(&mut self.filter_string);
                if txt_field.gained_focus() {
                    self.are_tools_active = false;
                }
                if txt_field.lost_focus() {
                    self.are_tools_active = true;
                }
                if txt_field.changed() {
                    handle_error!(
                        self.control.paths_navigator.filter(&self.filter_string),
                        self
                    );
                }

                // scroll area showing image file names
                let scroll_to_selected = self.control.paths_navigator.scroll_to_selected_label();
                let mut file_label_selected_idx =
                    self.control.paths_navigator.file_label_selected_idx();
                if let Some(ps) = &self.control.paths_navigator.paths_selector() {
                    self.scroll_offset = menu::scroll_area::scroll_area(
                        ui,
                        &mut file_label_selected_idx,
                        ps,
                        scroll_to_selected,
                        self.scroll_offset,
                    );
                    self.control
                        .paths_navigator
                        .deactivate_scroll_to_selected_label();
                    if self.control.paths_navigator.file_label_selected_idx()
                        != file_label_selected_idx
                    {
                        self.control
                            .paths_navigator
                            .select_label_idx(file_label_selected_idx);
                    }
                }

                ui.separator();
                ui.hyperlink_to("license and code", "https://github.com/bertiqwerty/rvimage");
            });
        if let (Some(wr), Some(pos)) = (window_response, ctx.pointer_latest_pos()) {
            if wr.response.rect.expand(5.0).contains(pos) {
                self.are_tools_active = false;
            } else {
                self.are_tools_active = true;
            }
        }
    }
}
