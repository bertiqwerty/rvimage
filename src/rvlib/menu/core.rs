use crate::{
    cfg::{self, Cfg},
    menu::{self, cfg_menu::CfgMenu},
    reader::{LoadImageForGui, ReaderFromCfg},
    threadpool::ThreadPool,
};
use egui::{ClippedMesh, CtxRef, Id, Response, Ui};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use image::DynamicImage;
use pixels::{wgpu, PixelsContext};
use winit::window::Window;

use super::paths_navigator::PathsNavigator;

/// Manages all state required for rendering egui over `Pixels`.
pub struct Framework {
    // State for egui.
    egui_ctx: CtxRef,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    rpass: RenderPass,
    paint_jobs: Vec<ClippedMesh>,

    // State for the GUI
    menu: Menu,
}

impl Framework {
    /// Create egui.
    pub fn new(width: u32, height: u32, scale_factor: f32, pixels: &pixels::Pixels) -> Self {
        let egui_ctx = CtxRef::default();
        let egui_state = egui_winit::State::from_pixels_per_point(scale_factor);
        let screen_descriptor = ScreenDescriptor {
            physical_width: width,
            physical_height: height,
            scale_factor,
        };
        let rpass = RenderPass::new(pixels.device(), pixels.render_texture_format(), 1);
        let menu = Menu::new();

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            rpass,
            paint_jobs: Vec::new(),
            menu,
        }
    }

    /// Handle input events from the window manager.
    pub fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        self.egui_state.on_event(&self.egui_ctx, event);
    }

    /// Resize egui.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.physical_width = width;
            self.screen_descriptor.physical_height = height;
        }
    }

    /// Update scaling factor.
    pub fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.scale_factor = scale_factor as f32;
    }

    /// Prepare egui.
    pub fn prepare(&mut self, window: &Window) {
        // Run the egui frame and create all paint jobs to prepare for rendering.
        let raw_input = self.egui_state.take_egui_input(window);
        let (output, paint_commands) = self.egui_ctx.run(raw_input, |egui_ctx| {
            // Draw the demo application.
            self.menu.ui(egui_ctx);
        });

        self.egui_state
            .handle_output(window, &self.egui_ctx, output);
        self.paint_jobs = self.egui_ctx.tessellate(paint_commands);
    }

    /// Render egui.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        render_target: &wgpu::TextureView,
        context: &PixelsContext,
    ) -> Result<(), BackendError> {
        // Upload all resources to the GPU.
        self.rpass
            .update_texture(&context.device, &context.queue, &self.egui_ctx.font_image());
        self.rpass
            .update_user_textures(&context.device, &context.queue);
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
        )
    }
    pub fn menu(&mut self) -> &mut Menu {
        &mut self.menu
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

pub struct Menu {
    window_open: bool, // Only show the egui window when true.
    reader: Option<ReaderFromCfg>,
    info_message: Info,
    filter_string: String,
    are_tools_active: bool,
    paths_navigator: PathsNavigator,
    cfg: Cfg,
    ssh_cfg_str: String,
    tp: ThreadPool<(ReaderFromCfg, Info)>,
    last_open_folder_job_id: Option<u128>,
}

impl Menu {
    fn new() -> Self {
        let (cfg, _) = get_cfg();
        let ssh_cfg_str = toml::to_string(&cfg.ssh_cfg).unwrap();
        Self {
            window_open: true,
            reader: None,
            info_message: Info::None,
            filter_string: "".to_string(),
            are_tools_active: true,
            paths_navigator: PathsNavigator::new(None),
            cfg,
            ssh_cfg_str,
            tp: ThreadPool::new(1),
            last_open_folder_job_id: None,
        }
    }

    pub fn popup(&mut self, info: Info) {
        self.info_message = info;
    }

    pub fn are_tools_active(&self) -> bool {
        self.are_tools_active
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

    pub fn file_label(&mut self, idx: usize) -> &str {
        match self.paths_navigator.paths_selector() {
            Some(ps) => ps.file_labels()[idx].1.as_str(),
            None => "",
        }
    }
    pub fn select_file_label(&mut self, file_label: &str) {
        self.paths_navigator
            .select_label_idx(self.idx_of_file_label(file_label));
    }

    pub fn read_image(&mut self, file_label_selected_idx: usize) -> Option<DynamicImage> {
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
                        r.read_image(file_label_selected_idx, &ffp)
                    }),
                self
            )
        }
        im_read
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &CtxRef) {
        egui::Window::new("menu")
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
                        self.cfg.clone(),
                        &mut self.last_open_folder_job_id,
                        &mut self.tp,
                    );
                    handle_error!(button_resp, self);
                    let popup_id = ui.make_persistent_id("cfg-popup");
                    let cfg_gui = CfgMenu::new(popup_id, &mut self.cfg, &mut self.ssh_cfg_str);
                    ui.add(cfg_gui);
                });

                // check if connection is after open folder is ready
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
                    handle_error!(
                        |ps| {
                            self.paths_navigator = PathsNavigator::new(ps);
                        },
                        {
                            self.reader
                                .as_ref()
                                .map_or(Ok(None), |r| r.open_folder().map(Some))
                        },
                        self
                    );
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
                    menu::scroll_area::scroll_area(
                        ui,
                        &mut file_label_selected_idx,
                        ps,
                        scroll_to_selected,
                    );
                    self.paths_navigator.deactivate_scroll_to_selected_label();
                    self.paths_navigator
                        .select_label_idx(file_label_selected_idx);
                }

                // help
                ui.separator();
                ui.label("zoom - drag left mouse");
                ui.label("move zoomed area - drag right mouse");
                ui.label("unzoom - backspace");
                ui.label("rotate by 90 degrees - r");
                ui.label("open or close this menu - m");
                ui.separator();
                ui.hyperlink_to("license and code", "https://github.com/bertiqwerty/rvimage");
            });
    }
}
