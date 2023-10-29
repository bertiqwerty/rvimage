#![deny(clippy::all)]
#![forbid(unsafe_code)]

use egui::{
    epaint::RectShape, Color32, ColorImage, Context, Event, Image, Pos2, Rect, Rounding, Shape,
    Stroke, TextureHandle, TextureOptions, Ui, Vec2,
};
use rvlib::{MainEventLoop, UpdateImage};

#[derive(Default)]
struct RvImageApp {
    event_loop: MainEventLoop,
    texture: Option<TextureHandle>,
    size: [usize; 2],
}

impl RvImageApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self::default()
    }
}

fn clrim_2_handle<'a>(color_image: ColorImage, ctx: &'a Context) -> TextureHandle {
    ctx.load_texture("canvas", color_image, TextureOptions::NEAREST)
}

fn handle_2_image<'a>(handle: &TextureHandle, ctx: &'a Context, size: [usize; 2]) -> Image<'a> {
    let size = egui::vec2(size[0] as f32, size[1] as f32);
    let sized_image = egui::load::SizedTexture::new(handle.id(), size);
    egui::Image::from_texture(sized_image)
}

fn rgb_2_clr(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

fn draw_bbs(ui: &mut Ui, bbs: &[rvlib::BB], stroke: rvlib::Stroke, fill_rgb: [u8; 3]) {
    let shapes = bbs
        .iter()
        .map(|bb| {
            let fill_rgb = rgb_2_clr(fill_rgb);
            let p = Pos2::new(bb.x as f32, bb.y as f32);
            let size = Vec2::new(bb.w as f32, bb.h as f32);
            let stroke = Stroke::new(stroke.thickness, rgb_2_clr(stroke.color));
            Shape::Rect(RectShape::new(
                Rect::from_min_size(p, size),
                Rounding::ZERO,
                fill_rgb,
                stroke,
            ))
        })
        .collect::<Vec<Shape>>();
    ui.painter().add(Shape::Vec(shapes));
}

impl eframe::App for RvImageApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Hello World!");
            if let Ok(update_view) = self
                .event_loop
                .one_iteration(&rvlib::Events::default(), ctx)
            {
                if let UpdateImage::Yes(im) = update_view.image {
                    let color_image = ColorImage::from_rgb(
                        [im.width() as usize, im.height() as usize],
                        im.as_raw(),
                    );
                    self.size = color_image.size;
                    self.texture = Some(clrim_2_handle(color_image, ctx));
                }
            }
            if let Some(texture) = self.texture.as_ref() {
                let ui_image = handle_2_image(texture, &ctx, self.size).shrink_to_fit();
                ui.add(ui_image);
            }
        });
    }
}

fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    // tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions::default();
    if let Err(e) = eframe::run_native(
        "RV Image",
        native_options,
        Box::new(|cc| Box::new(RvImageApp::new(cc))),
    ) {
        println!("{e:?}");
    }
}
