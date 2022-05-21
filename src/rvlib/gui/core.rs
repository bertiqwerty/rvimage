use crate::{
    cfg::{self, Cfg},
    gui::{self, cfg_gui::CfgGui},
    paths_selector::PathsSelector,
    reader::{LoadImageForGui, ReaderFromCfg},
    threadpool::ThreadPool,
};
use egui::{ClippedMesh, CtxRef, Id, Response, Ui};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use image::DynamicImage;
use pixels::{wgpu, PixelsContext};
use winit::window::Window;

fn next(file_selected_idx: usize, files_len: usize) -> usize {
    if file_selected_idx < files_len - 1 {
        file_selected_idx + 1
    } else {
        files_len - 1
    }
}

fn prev(file_selected_idx: usize, files_len: usize) -> usize {
    if file_selected_idx > files_len {
        files_len - 1
    } else if file_selected_idx > 0 {
        file_selected_idx - 1
    } else {
        0
    }
}

/// Manages all state required for rendering egui over `Pixels`.
pub struct Framework {
    // State for egui.
    egui_ctx: CtxRef,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    rpass: RenderPass,
    paint_jobs: Vec<ClippedMesh>,

    // State for the GUI
    gui: Gui,
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
        let gui = Gui::new();

        Self {
            egui_ctx,
            egui_state,
            screen_descriptor,
            rpass,
            paint_jobs: Vec::new(),
            gui,
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
            self.gui.ui(egui_ctx);
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
    pub fn gui(&mut self) -> &mut Gui {
        &mut self.gui
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
    egui::popup_below_widget(ui, popup_id, &below_respone, |ui| {
        let max_msg_len = 500;
        let shortened_msg = if msg.len() > max_msg_len {
            &msg[..max_msg_len]
        } else {
            &msg
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

pub struct Gui {
    window_open: bool, // Only show the egui window when true.
    reader: Option<ReaderFromCfg>,
    info_message: Info,
    filter_string: String,
    paths_selector: Option<PathsSelector>,
    file_label_selected_idx: Option<usize>,
    are_tools_active: bool,
    scroll_to_selected_label: bool,
    cfg: Cfg,
    ssh_cfg_str: String,
    tp: ThreadPool<(ReaderFromCfg, Info)>,
    last_open_folder_job_id: Option<usize>,
}

impl Gui {
    fn new() -> Self {
        let (cfg, _) = get_cfg();

        let ssh_cfg_str = toml::to_string(&cfg.ssh_cfg).unwrap();
        Self {
            window_open: true,
            reader: None,
            info_message: Info::None,
            filter_string: "".to_string(),
            paths_selector: None,
            file_label_selected_idx: None,
            are_tools_active: true,
            scroll_to_selected_label: false,
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

    pub fn next(&mut self) {
        match (self.file_label_selected_idx, &self.paths_selector) {
            (Some(idx), Some(ps)) => {
                self.file_label_selected_idx = Some(next(idx, ps.file_labels().len()));
                self.scroll_to_selected_label = true;
            }
            _ => (),
        }
    }

    pub fn prev(&mut self) {
        match (self.file_label_selected_idx, &self.paths_selector) {
            (Some(idx), Some(ps)) => {
                self.file_label_selected_idx = Some(prev(idx, ps.file_labels().len()));
                self.scroll_to_selected_label = true;
            }
            _ => (),
        }
    }

    pub fn toggle(&mut self) {
        if self.window_open {
            self.window_open = false;
        } else {
            self.window_open = true;
        }
    }

    pub fn file_label_selected_idx(&self) -> Option<usize> {
        self.file_label_selected_idx
    }

    pub fn file_label(&mut self, idx: usize) -> &str {
        match &self.paths_selector {
            Some(ps) => ps.file_labels()[idx].1.as_str(),
            None => "",
        }
    }

    pub fn read_image(&mut self, file_label_selected_idx: usize) -> Option<DynamicImage> {
        let mut im_read = None;
        self.reader.as_mut().map(|r| {
            handle_error!(
                |im| {
                    im_read = im;
                },
                {
                    match &self.paths_selector {
                        Some(ps) => {
                            let ffp = ps.filtered_file_paths();
                            r.read_image(file_label_selected_idx, &ffp)
                        }
                        None => Ok(None),
                    }
                },
                self
            )
        });
        im_read
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &CtxRef) {
        egui::Window::new("menu")
            .vscroll(true)
            .open(&mut self.window_open)
            .show(ctx, |ui| {
                optick::event!("show ctx");
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
                    optick::event!("top row buttons");
                    let button_resp = gui::open_folder::button(
                        ui,
                        &mut self.paths_selector,
                        self.cfg.clone(),
                        &mut self.last_open_folder_job_id,
                        &mut self.tp,
                    );
                    handle_error!(button_resp, self);
                    let popup_id = ui.make_persistent_id("cfg-popup");
                    let cfg_gui = CfgGui::new(popup_id, &mut self.cfg, &mut self.ssh_cfg_str);
                    ui.add(cfg_gui);
                });

                // check if connection is after open folder is ready
                let mut assign_open_folder_res = |reader_n_info: Option<(ReaderFromCfg, Info)>| {
                    optick::event!("assign open folder res");
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
                    gui::open_folder::check_if_connected(
                        ui,
                        &mut self.last_open_folder_job_id,
                        &self.paths_selector,
                        &mut self.tp,
                    ),
                    self
                );
                if self.paths_selector.is_none() {
                    handle_error!(
                        |ps| {
                            self.paths_selector = ps;
                        },
                        {
                            optick::event!("r.open_folder");
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
                    optick::event!("tf.changed");
                    self.paths_selector.as_mut().map(|ps| {
                        let unfiltered_idx_before_filter =
                            if let Some(filtered_idx) = self.file_label_selected_idx {
                                self.scroll_to_selected_label = true;
                                let (unfiltered_idx, _) = ps.file_labels()[filtered_idx];
                                Some(unfiltered_idx)
                            } else {
                                None
                            };
                        handle_error!(ps.filter(self.filter_string.trim()), self);
                        self.file_label_selected_idx = match unfiltered_idx_before_filter {
                            Some(unfiltered_idx) => {
 ps
                                    .file_labels()
                                    .iter()
                                    .enumerate()
                                    .find(|(_, (uidx, _))| *uidx == unfiltered_idx)
                                    .map(|(fidx, _)| fidx)
                            }
                            None => None,
                        };
                    });
                }

                // scroll area showing image file names
                let scroll_height = ui.available_height() - 120.0;
                if let Some(ps) = &self.paths_selector {
                    egui::ScrollArea::vertical()
                        .max_height(scroll_height)
                        .show(ui, |ui| {
                            handle_error!(
                                gui::scroll_area::scroll_area(
                                    ui,
                                    &mut self.file_label_selected_idx,
                                    ps,
                                    &mut self.scroll_to_selected_label,
                                ),
                                self
                            );
                        });
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
