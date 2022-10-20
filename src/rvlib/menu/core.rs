use crate::{
    cfg::{self, Cfg},
    menu::{self, cfg_menu::CfgMenu},
    reader::{LoadImageForGui, ReaderFromCfg},
    threadpool::ThreadPool,
    tools::ToolState,
};
use egui::{ClippedPrimitive, Context, Id, Response, TexturesDelta, Ui};
use egui_wgpu::renderer::{RenderPass, ScreenDescriptor};
use image::DynamicImage;
use pixels::{wgpu, PixelsContext};
use winit::window::Window;

use super::{open_folder::OpenFolder, paths_navigator::PathsNavigator};

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
    tools_menu: ToolsMenu,
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
        let tools_menu = ToolsMenu::new();
        let textures = TexturesDelta::default();
        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            rpass,
            paint_jobs: Vec::new(),
            textures,
            menu,
            tools_menu,
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
    pub fn prepare(&mut self, window: &Window, tools: &mut [ToolState]) {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        let raw_input = self.egui_state.take_egui_input(window);
        let output = self.egui_ctx.run(raw_input, |egui_ctx| {
            // Draw the demo application.
            self.menu.ui(egui_ctx);
            self.tools_menu.ui(egui_ctx, tools);
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
        self.menu.are_tools_active && self.tools_menu.are_tools_active
    }
    pub fn activated_tool(&self) -> Option<usize> {
        self.tools_menu.activated_tool
    }
    pub fn toggle_tools_menu(&mut self) {
        self.tools_menu.toggle();
    }
}

#[derive(Clone, Debug)]
pub enum Info {
    Error(String),
    Warning(String),
    None,
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

pub fn get_cfg() -> (Cfg, Info) {
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

pub struct ToolsMenu {
    window_open: bool,      // Only show the egui window when true.
    are_tools_active: bool, // can deactivate all tools, overrides activated_tool
    activated_tool: Option<usize>,
}
impl ToolsMenu {
    fn new() -> Self {
        Self {
            window_open: true,
            are_tools_active: true,
            activated_tool: None,
        }
    }
    fn ui(&mut self, ctx: &Context, tools: &mut [ToolState]) {
        let window_response = egui::Window::new("tools")
            .vscroll(true)
            .title_bar(false)
            .open(&mut self.window_open)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    self.activated_tool = tools
                        .iter_mut()
                        .enumerate()
                        .find(|(_, t)| ui.selectable_label(t.is_active(), t.button_label).clicked())
                        .map(|(i, _)| i);
                })
            });
        if let Some(wr) = window_response {
            if wr.response.hovered() {
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
    window_open: bool, // Only show the egui window when true.
    reader: Option<ReaderFromCfg>,
    info_message: Info,
    filter_string: String,
    opened_folder: OpenFolder,
    are_tools_active: bool,
    paths_navigator: PathsNavigator,
    cfg: Cfg,
    ssh_cfg_str: String,
    tp: ThreadPool<(ReaderFromCfg, Info)>,
    last_open_folder_job_id: Option<u128>,
    scroll_offset: f32,
}

impl Menu {
    fn new() -> Self {
        let (cfg, _) = get_cfg();
        let ssh_cfg_str = toml::to_string_pretty(&cfg.ssh_cfg).unwrap();
        Self {
            window_open: true,
            reader: None,
            info_message: Info::None,
            filter_string: "".to_string(),
            opened_folder: OpenFolder::None,
            are_tools_active: true,
            paths_navigator: PathsNavigator::new(None),
            cfg,
            ssh_cfg_str,
            tp: ThreadPool::new(1),
            last_open_folder_job_id: None,
            scroll_offset: 0.0,
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
        self.paths_navigator.prev();
    }
    pub fn next(&mut self) {
        self.paths_navigator.next();
    }
    pub fn file_label_selected_idx(&self) -> Option<usize> {
        self.paths_navigator.file_label_selected_idx()
    }

    fn idx_of_file_label(&self, file_label: &str) -> Option<usize> {
        match self.paths_navigator.paths_selector() {
            Some(ps) => ps.idx_of_file_label(file_label),
            None => None,
        }
    }

    pub fn file_label(&self, idx: usize) -> &str {
        match self.paths_navigator.paths_selector() {
            Some(ps) => ps.file_labels()[idx].1.as_str(),
            None => "",
        }
    }
    pub fn select_file_label(&mut self, file_label: &str) {
        self.paths_navigator
            .select_label_idx(self.idx_of_file_label(file_label));
    }
    pub fn activate_scroll_to_label(&mut self) {
        self.paths_navigator.activate_scroll_to_selected_label();
    }
    pub fn select_label_idx(&mut self, file_label_idx: Option<usize>) {
        self.paths_navigator.select_label_idx(file_label_idx);
    }

    pub fn folder_label(&self) -> Option<&str> {
        self.paths_navigator
            .paths_selector()
            .as_ref()
            .map(|ps| ps.folder_label())
    }

    pub fn file_path(&self, file_idx: usize) -> Option<&str> {
        self.paths_navigator
            .paths_selector()
            .as_ref()
            .map(|ps| ps.file_selected_path(file_idx))
    }

    pub fn read_image(
        &mut self,
        file_label_selected_idx: usize,
        reload: bool,
    ) -> Option<DynamicImage> {
        let mut im_read = None;
        if let Some(r) = &mut self.reader {
            handle_error!(
                |im| {
                    im_read = im;
                },
                self.paths_navigator
                    .paths_selector()
                    .as_ref()
                    .map_or(Ok(None), |ps| {
                        let ffp = ps.filtered_file_paths();
                        r.read_image(file_label_selected_idx, &ffp, reload)
                    }),
                self
            )
        }
        im_read
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &Context) {
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
                    let button_resp = menu::open_folder::button(
                        ui,
                        &mut self.paths_navigator,
                        std::mem::replace(&mut self.opened_folder, OpenFolder::None),
                        self.cfg.clone(),
                        &mut self.last_open_folder_job_id,
                        &mut self.tp,
                    );
                    handle_error!(
                        |folder| {
                            self.opened_folder = folder;
                        },
                        button_resp,
                        self
                    );
                    let popup_id = ui.make_persistent_id("cfg-popup");
                    let cfg_gui = CfgMenu::new(popup_id, &mut self.cfg, &mut self.ssh_cfg_str);
                    ui.add(cfg_gui);
                });

                // check if connection is ready after open folder
                let mut assign_open_folder_res = |reader_n_info: Option<(ReaderFromCfg, Info)>| {
                    if let Some((reader, info)) = reader_n_info {
                        self.reader = Some(reader);
                        match info {
                            Info::None => (),
                            _ => {
                                self.info_message = info;
                            }
                        }
                    }
                };
                handle_error!(
                    assign_open_folder_res,
                    menu::open_folder::check_if_connected(
                        ui,
                        &mut self.last_open_folder_job_id,
                        self.paths_navigator.paths_selector(),
                        &mut self.tp,
                    ),
                    self
                );
                if self.paths_navigator.paths_selector().is_none() {
                    if let OpenFolder::Some(open_folder) = &self.opened_folder {
                        handle_error!(
                            |ps| {
                                self.paths_navigator = PathsNavigator::new(ps);
                            },
                            {
                                self.reader
                                    .as_ref()
                                    .map_or(Ok(None), |r| r.open_folder(open_folder).map(Some))
                            },
                            self
                        );
                    }
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
                    handle_error!(self.paths_navigator.filter(&self.filter_string), self);
                }

                // scroll area showing image file names
                let scroll_to_selected = self.paths_navigator.scroll_to_selected_label();
                let mut file_label_selected_idx = self.paths_navigator.file_label_selected_idx();
                if let Some(ps) = &self.paths_navigator.paths_selector() {
                    self.scroll_offset = menu::scroll_area::scroll_area(
                        ui,
                        &mut file_label_selected_idx,
                        ps,
                        scroll_to_selected,
                        self.scroll_offset,
                    );
                    self.paths_navigator.deactivate_scroll_to_selected_label();
                    if self.paths_navigator.file_label_selected_idx() != file_label_selected_idx {
                        self.paths_navigator
                            .select_label_idx(file_label_selected_idx);
                    }
                }

                // help
                ui.separator();
                ui.label("activate zoom tool - shift+z");
                ui.label("  zoom - drag left mouse");
                ui.label("  move zoomed area - drag right mouse");
                ui.label("  unzoom - backspace");
                ui.label("activate rotate tool - shift+r");
                ui.label("  rotate by 90 degrees - r");
                ui.separator();
                ui.label("open or close this menu - m");
                ui.label("copy file path - right click on file label");
                ui.separator();
                ui.hyperlink_to("license and code", "https://github.com/bertiqwerty/rvimage");
            });
        if let Some(wr) = window_response {
            if wr.response.hovered() {
                self.are_tools_active = false;
            } else {
                self.are_tools_active = true;
            }
        }
    }
}
