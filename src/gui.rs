use crate::read::{FolderReader, ReadImageFiles};
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
    data_point: Option<(u32, u32, [u8; 3])>,
    buffer_size: (u32, u32),
    reader: RIF,
}

impl<RIF> Gui<RIF>
where
    RIF: ReadImageFiles,
{
    /// Create a `Gui`.
    fn new() -> Self {
        Self {
            window_open: true,
            data_point: None,
            buffer_size: (0, 0),
            reader: RIF::new(),
        }
    }
    pub fn next(&mut self) {
        self.reader.next();
    }
    pub fn prev(&mut self) {
        self.reader.prev();
    }
    pub fn set_state(&mut self, data_point: Option<(u32, u32, [u8; 3])>, buffer_size: (u32, u32)) {
        self.data_point = data_point;
        self.buffer_size = buffer_size;
    }
    pub fn file_selected_idx(&self) -> Option<usize> {
        self.reader.file_selected_idx()
    }

    pub fn read_image(&self, file_selected: usize) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        self.reader.read_image(file_selected)
    }

    /// Create the UI using egui.
    fn ui(&mut self, ctx: &CtxRef) {
        egui::Window::new("menu")
            .vscroll(true)
            .open(&mut self.window_open)
            .show(ctx, |ui| {
                ui.label(format!(
                    "(width, height) = ({}, {})",
                    self.buffer_size.0, self.buffer_size.1
                ));
                ui.label(match self.data_point {
                    Some((x, y, rgb)) => {
                        format!("({}, {}) -> ({}, {}, {})", x, y, rgb[0], rgb[1], rgb[2])
                    }
                    None => "(x, y) -> (r, g, b)".to_string(),
                });
                ui.separator();
                if ui.button("open folder...").clicked() {
                    self.reader.open_folder();
                }

                ui.label(self.reader.folder_label());
                ui.label(self.reader.file_selected_label());

                for (idx, s) in self.reader.list_file_labels().iter().enumerate() {
                    if ui
                        .selectable_label(false, s)
                        .clicked()
                    {
                        self.reader.selected_file(idx);
                    };
                }
                ui.separator();
                ui.label("crop - left mouse");
                ui.label("move crop - right mouse");
                ui.label("uncrop - backspace");
                ui.separator();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x /= 2.0;
                    ui.label("Learn more about egui at");
                    ui.hyperlink("https://docs.rs/egui");
                });
            });
    }
}
