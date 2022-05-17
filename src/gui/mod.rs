use crate::{
    cfg::{self, Cfg},
    gui::cfg_gui::CfgGui,
    reader::{LoadImageForGui, ReaderFromCfg},
};
use egui::{Align, ClippedMesh, CtxRef};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use image::DynamicImage;
use pixels::{wgpu, PixelsContext};
use winit::window::Window;
mod cfg_gui;
fn next(file_selected_idx: Option<usize>, files_len: usize) -> Option<usize> {
    file_selected_idx.map(|idx| {
        if idx < files_len - 1 {
            idx + 1
        } else {
            files_len - 1
        }
    })
}

fn prev(file_selected_idx: Option<usize>, files_len: usize) -> Option<usize> {
    file_selected_idx.map(|idx| {
        if idx > files_len {
            files_len - 1
        } else if idx > 0 {
            idx - 1
        } else {
            0
        }
    })
}

/// Returns the gui-idx (not the remote idx) and the label of the selected file
fn find_selected_remote_idx(
    selected_remote_idx: usize,
    file_labels: &[(usize, String)],
) -> Option<(usize, &str)> {
    file_labels
        .iter()
        .enumerate()
        .find(|(_, (r_idx, _))| selected_remote_idx == *r_idx)
        .map(|(g_idx, (_, label))| (g_idx, label.as_str()))
}

/// Manages all state required for rendering egui over `Pixels`.
pub(crate) struct Framework {
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
    pub(crate) fn new(width: u32, height: u32, scale_factor: f32, pixels: &pixels::Pixels) -> Self {
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
    pub(crate) fn handle_event(&mut self, event: &winit::event::WindowEvent) {
        self.egui_state.on_event(&self.egui_ctx, event);
    }

    /// Resize egui.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.screen_descriptor.physical_width = width;
            self.screen_descriptor.physical_height = height;
        }
    }

    /// Update scaling factor.
    pub(crate) fn scale_factor(&mut self, scale_factor: f64) {
        self.screen_descriptor.scale_factor = scale_factor as f32;
    }

    /// Prepare egui.
    pub(crate) fn prepare(&mut self, window: &Window) {
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
    pub(crate) fn render(
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

#[derive(Clone)]
pub enum Info {
    Error(String),
    Warning(String),
    None,
}

fn get_cfg() -> (Cfg, Info) {
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
}

fn make_reader_from_cfg() -> (ReaderFromCfg, Info) {
    match ReaderFromCfg::new() {
        Ok(rfc) => (rfc, Info::None),
        Err(e) => (
            ReaderFromCfg::new().expect("default cfg broken"),
            Info::Warning(e.msg().to_string()),
        ),
    }
}
pub struct Gui {
    window_open: bool, // Only show the egui window when true.
    reader: ReaderFromCfg,
    info_message: Info,
    file_labels: Vec<(usize, String)>,
    filter_string: String,
    file_selected_idx: Option<usize>,
    are_tools_active: bool,
    scroll_to_selected_label: bool,
    cfg: Cfg,
    ssh_cfg: String,
}

impl Gui {
    fn new() -> Self {
        let (reader_from_cfg, info) = make_reader_from_cfg();
        let (cfg, _) = get_cfg();

        let ssh_cfg_str = toml::to_string(&cfg.ssh_cfg).unwrap();
        Self {
            window_open: true,
            reader: reader_from_cfg,
            info_message: info,
            file_labels: vec![],
            filter_string: "".to_string(),
            file_selected_idx: None,
            are_tools_active: true,
            scroll_to_selected_label: false,
            cfg,
            ssh_cfg: ssh_cfg_str,
        }
    }

    pub fn popup(&mut self, info: Info) {
        self.info_message = info;
    }

    pub fn are_tools_active(&self) -> bool {
        self.are_tools_active
    }

    pub fn next(&mut self) {
        self.file_selected_idx = next(self.file_selected_idx, self.file_labels.len());
        if let Some(idx) = self.file_selected_idx {
            self.reader.select_file(self.file_labels[idx].0);
            self.scroll_to_selected_label = true;
        }
    }

    pub fn prev(&mut self) {
        self.file_selected_idx = prev(self.file_selected_idx, self.file_labels.len());
        if let Some(idx) = self.file_selected_idx {
            self.reader.select_file(self.file_labels[idx].0);
            self.scroll_to_selected_label = true;
        }
    }

    pub fn open(&mut self) {
        self.window_open = true;
    }

    pub fn file_selected_idx(&self) -> Option<usize> {
        self.reader.file_selected_idx()
    }

    pub fn file_label(&mut self, remote_idx: usize) -> &str {
        match find_selected_remote_idx(remote_idx, &self.file_labels) {
            Some((_, label)) => label,
            None => "",
        }
    }

    pub fn read_image(&mut self, file_selected: usize) -> Option<DynamicImage> {
        let mut im_read = None;
        handle_error!(
            |im| { im_read = im },
            self.reader.read_image(file_selected),
            self
        );
        im_read
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &CtxRef) {
        egui::Window::new("menu")
            .vscroll(true)
            .open(&mut self.window_open)
            .show(ctx, |ui| {
                let popup_id = ui.make_persistent_id("info-popup");
                let r = ui.separator();
                let show_popup = |msg: &str, icon: &str| {
                    ui.memory().open_popup(popup_id);
                    let mut new_msg = Info::None;
                    egui::popup_below_widget(ui, popup_id, &r, |ui| {
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
                            self.info_message.clone()
                        }
                    });
                    new_msg
                };
                self.info_message = match &self.info_message {
                    Info::Warning(msg) => show_popup(msg, "❕"),
                    Info::Error(msg) => show_popup(msg, "❌"),
                    Info::None => Info::None,
                };
                if ui.button("open folder").clicked() {
                    let reader_info_tmp = make_reader_from_cfg();
                    self.reader = reader_info_tmp.0;
                    self.info_message = reader_info_tmp.1;
                    handle_error!(|_| (), self.reader.open_folder(), self);
                    handle_error!(
                        |v| { self.file_labels = v },
                        self.reader.list_file_labels(""),
                        self
                    );
                }

                let mut ui_label = |s| ui.label(s);
                handle_error!(ui_label, self.reader.folder_label(), self);

                let txt_field = ui.text_edit_singleline(&mut self.filter_string);
                if txt_field.gained_focus() {
                    self.are_tools_active = false;
                }
                if txt_field.lost_focus() {
                    self.are_tools_active = true;
                }
                if txt_field.changed() {
                    let new_labels = self.reader.list_file_labels(self.filter_string.trim());
                    if let Ok(nl) = &new_labels {
                        if let Some(gui_idx) = self.file_selected_idx {
                            let selected_remote_idx = self.file_labels[gui_idx].0;
                            self.file_selected_idx =
                                find_selected_remote_idx(selected_remote_idx, nl)
                                    .map(|(gui_idx, _)| gui_idx);
                            if self.file_selected_idx.is_some() {
                                self.scroll_to_selected_label = true;
                            }
                        }
                    }
                    handle_error!(|v| { self.file_labels = v }, new_labels, self);
                }
                let scroll_height = ui.available_height() - 120.0;
                egui::ScrollArea::vertical()
                    .max_height(scroll_height)
                    .show(ui, |ui| {
                        for (idx, (reader_idx, s)) in self.file_labels.iter().enumerate() {
                            let sl = if self.file_selected_idx == Some(idx) {
                                let mut path = "".to_string();
                                handle_error!(
                                    |v| {
                                        path = v;
                                    },
                                    self.reader.file_selected_path(),
                                    self
                                );

                                let sl_ = ui.selectable_label(true, s).on_hover_text(path);
                                if self.scroll_to_selected_label {
                                    sl_.scroll_to_me(Align::Center);
                                }
                                sl_
                            } else {
                                ui.selectable_label(false, s)
                            };
                            if sl.clicked() {
                                self.reader.select_file(*reader_idx);
                                self.file_selected_idx = Some(idx);
                            }
                        }
                        self.scroll_to_selected_label = false;
                    });
                let popup_id = ui.make_persistent_id("cfg-popup");
                let cfg_gui = CfgGui::new(popup_id, &mut self.cfg, &mut self.ssh_cfg);
                ui.add(cfg_gui);
                ui.separator();
                ui.label("zoom - drag left mouse");
                ui.label("move zoomed area - drag right mouse");
                ui.label("unzoom - backspace");
                ui.label("r - rotate by 90 degrees");
                ui.label("open this menu - m");
            });
    }
}
