use crate::reader::{FolderReader, ReadImageFiles};
use egui::{ClippedMesh, CtxRef};
use egui_wgpu_backend::{BackendError, RenderPass, ScreenDescriptor};
use image::{ImageBuffer, Rgb};
use pixels::{wgpu, PixelsContext};
use winit::window::Window;

/// Manages all state required for rendering egui over `Pixels`.
pub(crate) struct Framework {
    // State for egui.
    egui_ctx: CtxRef,
    egui_state: egui_winit::State,
    screen_descriptor: ScreenDescriptor,
    rpass: RenderPass,
    paint_jobs: Vec<ClippedMesh>,

    // State for the GUI
    gui: Gui<FolderReader>,
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
    pub fn gui(&mut self) -> &mut Gui<FolderReader> {
        &mut self.gui
    }
}

/// Example application state. A real application will need a lot more state than this.
pub struct Gui<RIF>
where
    RIF: ReadImageFiles,
{
    /// Only show the egui window when true.
    window_open: bool,
    reader: RIF,
    error_message: Option<String>,
}

macro_rules! handle_error {
    ($effect:expr, $result:expr, $self:expr) => {
        match $result {
            Ok(r) => {
                $effect(r);
            }
            Err(e) => {
                $self.error_message = Some(e.to_string());
            }
        }
    };
}

impl<RIF> Gui<RIF>
where
    RIF: ReadImageFiles,
{
    /// Create a `Gui`.
    fn new() -> Self {
        Self {
            window_open: true,
            reader: RIF::new(),
            error_message: None,
        }
    }

    pub fn next(&mut self) {
        self.reader.next();
    }

    pub fn prev(&mut self) {
        self.reader.prev();
    }

    pub fn open(&mut self) {
        self.window_open = true;
    }

    pub fn file_selected_idx(&self) -> Option<usize> {
        self.reader.file_selected_idx()
    }

    pub fn read_image(&mut self, file_selected: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        let mut im_read = ImageBuffer::new(0, 0);
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
                match &self.error_message {
                    Some(msg) => {
                        ui.label(format!("❌ {}", msg));
                    }
                    None => (),
                }
                ui.separator();
                if ui.button("open folder...").clicked() {
                    handle_error!(|_| (), self.reader.open_folder(), self);
                }

                let mut ui_label = |s| ui.label(s);
                handle_error!(ui_label, self.reader.folder_label(), self);
                handle_error!(ui_label, self.reader.file_selected_label(), self);

                let mut file_labels = vec![];
                handle_error!(
                    |v| { file_labels = v },
                    self.reader.list_file_labels(),
                    self
                );
                for (idx, s) in file_labels.iter().enumerate() {
                    if ui.selectable_label(false, s).clicked() {
                        self.reader.selected_file(idx);
                    };
                }
                ui.separator();
                ui.label("crop - left mouse");
                ui.label("move crop - right mouse");
                ui.label("uncrop - backspace");
                ui.label("open this menu - m");
            });
    }
}
