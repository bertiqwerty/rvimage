#![deny(clippy::all)]
#![forbid(unsafe_code)]

use egui::{
    epaint::RectShape, Color32, ColorImage, Context, Image, Pos2, Rect, Response, Rounding, Sense,
    Shape, Stroke, TextureHandle, TextureOptions, Ui, Vec2,
};
use rvlib::{
    domain::Point, Annotation, Events, GeoFig, KeyCode, MainEventLoop, UpdateAnnos, UpdateImage,
};
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

fn clrim_2_handle<'a>(color_image: ColorImage, ctx: &'a Context) -> TextureHandle {
    ctx.load_texture("canvas", color_image, TextureOptions::NEAREST)
}

fn handle_2_image<'a>(handle: &TextureHandle, size: [usize; 2]) -> Image<'a> {
    let size = egui::vec2(size[0] as f32, size[1] as f32);
    let sized_image = egui::load::SizedTexture::new(handle.id(), size);
    egui::Image::from_texture(sized_image)
}

fn rgb_2_clr(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], 100)
}

fn draw_annos(ui: &mut Ui, annos: &[Annotation]) {
    let shapes = annos
        .iter()
        .map(|anno| {
            let bb = match &anno.geofig {
                GeoFig::BB(bb) => *bb,
                // TODO: draw actual polygon
                GeoFig::Poly(poly) => poly.enclosing_bb(),
            };
            let fill_rgb = rgb_2_clr(anno.fill_color);
            let p = Pos2::new(bb.x as f32, bb.y as f32);
            let size = Vec2::new(bb.w as f32, bb.h as f32);
            let stroke = Stroke::new(anno.outline.thickness, rgb_2_clr(anno.outline.color));
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

fn map_key_events(ui: &mut Ui) -> Vec<rvlib::Event> {
    let mut events = vec![];
    ui.input(|i| {
        for e in i.events.iter() {
            match e {
                egui::Event::Key {
                    key,
                    pressed,
                    repeat: _,
                    modifiers,
                } => {
                    if let Some(k) = map_key(*key) {
                        if !pressed {
                            events.push(rvlib::Event::Released(k));
                        } else {
                            events.push(rvlib::Event::Pressed(k));
                        }
                    }
                    if modifiers.alt {
                        events.push(rvlib::Event::Pressed(KeyCode::Alt));
                    }
                    if modifiers.command {
                        events.push(rvlib::Event::Pressed(KeyCode::Ctrl));
                    }
                    if modifiers.shift {
                        events.push(rvlib::Event::Pressed(KeyCode::Shift));
                    }
                }
                _ => (),
            }
        }
    });
    events
}

fn map_mouse_events(image_response: &Response) -> Vec<rvlib::Event> {
    let mut events = vec![];
    if image_response.clicked() || image_response.drag_released() {
        events.push(rvlib::Event::Released(KeyCode::MouseLeft));
    }
    if image_response.drag_started() {
        events.push(rvlib::Event::Pressed(KeyCode::MouseLeft));
    }
    events
}
#[derive(Default)]
struct RvImageApp {
    event_loop: MainEventLoop,
    texture: Option<TextureHandle>,
    annos: Vec<Annotation>,
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
    fn draw_image(&mut self, ui: &mut Ui, ctx: &Context, update_image: &UpdateImage) {
        if let UpdateImage::Yes(im) = &update_image {
            let color_image =
                ColorImage::from_rgb([im.width() as usize, im.height() as usize], im.as_raw());
            self.size = color_image.size;
            self.texture = Some(clrim_2_handle(color_image, ctx));
        }

        if let Some(texture) = self.texture.as_ref() {
            let ui_image = handle_2_image(texture, self.size)
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
            let key_events = map_key_events(ui);
            let mouse_events = map_mouse_events(&image_response);

            self.events = mem::take(&mut self.events)
                .events(key_events)
                .events(mouse_events)
                .mousepos(mouse_pos);
        }
    }
    fn draw_annos(&mut self, ui: &mut Ui, update_annos: UpdateAnnos) {
        if let UpdateAnnos::Yes(annos) = update_annos {
            self.annos = annos;
        }
        if !self.annos.is_empty() {
            draw_annos(ui, &self.annos);
        }
    }
}

impl eframe::App for RvImageApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let update_view = self.event_loop.one_iteration(&self.events, ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Ok(update_view) = update_view {
                ui.label(&update_view.image_info);
                self.draw_image(ui, ctx, &update_view.image);
                self.draw_annos(ui, update_view.annos);
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
