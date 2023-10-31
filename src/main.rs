#![deny(clippy::all)]
#![forbid(unsafe_code)]

use egui::{
    epaint::RectShape, Color32, ColorImage, Context, Image, Pos2, Rect, Rounding, Sense, Shape,
    Stroke, TextureHandle, TextureOptions, Ui, Vec2,
};
use rvlib::{domain::Point, Events, KeyCode, MainEventLoop, UpdateImage};
use std::mem;

fn map_key(egui_key: egui::Key) -> Option<rvlib::KeyCode> {
    match egui_key {
        egui::Key::A => Some(rvlib::KeyCode::A),
        egui::Key::B => Some(rvlib::KeyCode::B),
        egui::Key::C => Some(rvlib::KeyCode::C),
        egui::Key::D => Some(rvlib::KeyCode::D),
        egui::Key::L => Some(rvlib::KeyCode::L),
        egui::Key::H => Some(rvlib::KeyCode::H),
        egui::Key::M => Some(rvlib::KeyCode::M),
        egui::Key::Q => Some(rvlib::KeyCode::Q),
        egui::Key::R => Some(rvlib::KeyCode::R),
        egui::Key::T => Some(rvlib::KeyCode::T),
        egui::Key::V => Some(rvlib::KeyCode::V),
        egui::Key::Y => Some(rvlib::KeyCode::Y),
        egui::Key::Z => Some(rvlib::KeyCode::Z),
        egui::Key::Num0 => Some(rvlib::KeyCode::Key0),
        egui::Key::Num1 => Some(rvlib::KeyCode::Key1),
        egui::Key::Num2 => Some(rvlib::KeyCode::Key2),
        egui::Key::Num3 => Some(rvlib::KeyCode::Key3),
        egui::Key::Num4 => Some(rvlib::KeyCode::Key4),
        egui::Key::Num5 => Some(rvlib::KeyCode::Key5),
        egui::Key::Num6 => Some(rvlib::KeyCode::Key6),
        egui::Key::Num7 => Some(rvlib::KeyCode::Key7),
        egui::Key::Num8 => Some(rvlib::KeyCode::Key8),
        egui::Key::Num9 => Some(rvlib::KeyCode::Key9),
        egui::Key::PlusEquals => Some(rvlib::KeyCode::Equals),
        egui::Key::Minus => Some(rvlib::KeyCode::Minus),
        egui::Key::Delete => Some(rvlib::KeyCode::Delete),
        egui::Key::Backspace => Some(rvlib::KeyCode::Back),
        egui::Key::ArrowLeft => Some(rvlib::KeyCode::Left),
        egui::Key::ArrowRight => Some(rvlib::KeyCode::Right),
        egui::Key::ArrowUp => Some(rvlib::KeyCode::Up),
        egui::Key::ArrowDown => Some(rvlib::KeyCode::Down),
        egui::Key::F5 => Some(rvlib::KeyCode::F5),
        egui::Key::PageDown => Some(rvlib::KeyCode::PageDown),
        egui::Key::PageUp => Some(rvlib::KeyCode::PageUp),
        egui::Key::Escape => Some(rvlib::KeyCode::Escape),
        _ => None,
    }
}

#[derive(Default)]
struct RvImageApp {
    event_loop: MainEventLoop,
    texture: Option<TextureHandle>,
    size: [usize; 2],
    events: Events,
}

impl RvImageApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
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
        let update_view = self.event_loop.one_iteration(&self.events, ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.input(|i| {
                self.events.set_events(
                    i.events
                        .iter()
                        .flat_map(move |e| match e {
                            egui::Event::Key {
                                key,
                                pressed,
                                repeat,
                                modifiers,
                            } => {
                                if let Some(k) = map_key(*key) {
                                    if !pressed {
                                        Some(rvlib::Event::Released(k))
                                    } else {
                                        Some(rvlib::Event::Pressed(k))
                                    }
                                    
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>(),
                );
            });
            if let Ok(update_view) = update_view {
                ui.label(update_view.image_info);
                if let UpdateImage::Yes(im) = update_view.image {
                    let color_image = ColorImage::from_rgb(
                        [im.width() as usize, im.height() as usize],
                        im.as_raw(),
                    );
                    self.size = color_image.size;
                    self.texture = Some(clrim_2_handle(color_image, ctx));
                }

                if let Some(texture) = self.texture.as_ref() {
                    let ui_image = handle_2_image(texture, &ctx, self.size)
                        .shrink_to_fit()
                        .sense(Sense::hover())
                        .sense(Sense::click_and_drag());

                    let image_response = ui.add(ui_image);
                    let size = image_response.rect.size();
                    let offset_x = image_response.rect.min.x;
                    let offset_y = image_response.rect.min.y;
                    let mouse_pos = image_response.hover_pos();
                    let mouse_pos = mouse_pos.map(|mp| Point {
                        x: ((mp.x - offset_x) / size.x * self.size[0] as f32) as u32,
                        y: ((mp.y - offset_y) / size.y * self.size[1] as f32) as u32,
                    });
                    if image_response.clicked() {
                        self.events.released(KeyCode::MouseLeft);
                    }
                    if image_response.drag_released() {
                        self.events.released(KeyCode::MouseLeft);
                    }
                    if image_response.drag_started() {
                        self.events.pressed(KeyCode::MouseLeft);
                    }
                    self.events = mem::take(&mut self.events).mousepos(mouse_pos);
                }
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
