#![deny(clippy::all)]
#![forbid(unsafe_code)]

use egui::{
    epaint::RectShape, Color32, ColorImage, Context, Image, Pos2, Rect, Response, Rounding, Sense,
    Shape, Stroke, TextureHandle, TextureOptions, Ui, Vec2,
};
use image::{GenericImageView, ImageBuffer, Rgb};
use rvlib::{
    domain::{orig_pos_to_view_pos, view_pos_to_orig_pos},
    Annotation, GeoFig, KeyCode, MainEventLoop, UpdateAnnos, UpdateImage, UpdateZoomBox, BB,
};

type ImageU8 = ImageBuffer<Rgb<u8>, Vec<u8>>;

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
        egui::Key::PlusEquals => Some(rvlib::KeyCode::PlusEquals),
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

fn clrim_2_handle(color_image: ColorImage, ctx: &Context) -> TextureHandle {
    ctx.load_texture("canvas", color_image, TextureOptions::NEAREST)
}

fn handle_2_image<'a>(handle: &TextureHandle, size: [usize; 2]) -> Image<'a> {
    let size = egui::vec2(size[0] as f32, size[1] as f32);
    let sized_image = egui::load::SizedTexture::new(handle.id(), size);
    egui::Image::from_texture(sized_image)
}

fn rgb_2_clr(rgb: Option<[u8; 3]>) -> Color32 {
    if let Some(rgb) = rgb {
        Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], 100)
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 0)
    }
}

fn map_key_events(ui: &mut Ui) -> Vec<rvlib::Event> {
    let mut events = vec![];
    ui.input(|i| {
        for e in i.events.iter() {
            if let egui::Event::Key {
                key,
                pressed,
                repeat: _,
                modifiers,
            } = e
            {
                if let Some(k) = map_key(*key) {
                    if !pressed {
                        events.push(rvlib::Event::Released(k));
                    } else {
                        events.push(rvlib::Event::Pressed(k));
                    }
                }
                if modifiers.alt {
                    events.push(rvlib::Event::Held(KeyCode::Alt));
                }
                if modifiers.ctrl {
                    events.push(rvlib::Event::Held(KeyCode::Ctrl));
                }
                if modifiers.shift {
                    events.push(rvlib::Event::Held(KeyCode::Shift));
                }
            }
        }
    });
    events
}

fn map_mouse_events(image_response: &Response) -> Vec<rvlib::Event> {
    let mut events = vec![];
    if image_response.clicked() || image_response.drag_released() {
        println!("mouse left released");
        events.push(rvlib::Event::Released(KeyCode::MouseLeft));
    }
    if image_response.drag_started() {
        events.push(rvlib::Event::Pressed(KeyCode::MouseLeft));
    }
    if image_response.dragged() {
        events.push(rvlib::Event::Held(KeyCode::MouseLeft));
    }
    events
}

fn vec2_2_shape(v: Vec2) -> rvlib::Shape {
    rvlib::Shape::new(v.x as u32, v.y as u32)
}

fn image_2_colorimage(im: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ColorImage {
    ColorImage::from_rgb([im.width() as usize, im.height() as usize], im.as_raw())
}

fn orig_2_view(im_orig: &ImageU8, zoom_box: Option<BB>) -> ImageU8 {
    if let Some(zoom_box) = zoom_box {
        im_orig
            .view(zoom_box.x, zoom_box.y, zoom_box.w, zoom_box.h)
            .to_image()
    } else {
        im_orig.clone()
    }
}

#[derive(Default)]
struct RvImageApp {
    event_loop: MainEventLoop,
    texture: Option<TextureHandle>,
    annos: Vec<Annotation>,
    zoom_box: Option<BB>,
    im_orig: ImageU8,
    im_view: ImageU8,
    events: rvlib::Events,
}

impl RvImageApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self::default()
    }
    fn shape_orig(&self) -> rvlib::Shape {
        rvlib::Shape::from_im(&self.im_orig)
    }
    fn shape_view(&self) -> rvlib::Shape {
        rvlib::Shape::from_im(&self.im_view)
    }

    fn draw_annos(&self, ui: &mut Ui, image_rect: &Rect) {
        let shapes = self
            .annos
            .iter()
            .flat_map(|anno| {
                let bb = match &anno.geofig {
                    GeoFig::BB(bb) => *bb,
                    // TODO: draw actual polygon
                    GeoFig::Poly(poly) => poly.enclosing_bb(),
                };
                let fill_rgb = rgb_2_clr(anno.fill_color);
                let p = orig_pos_to_view_pos(
                    bb.min(),
                    self.shape_orig(),
                    self.shape_view(),
                    &self.zoom_box,
                );
                p.map(|p| {
                    // change between im_view and image_rect not yet taken into account
                    let p = Pos2 {
                        x: image_rect.min.x + p.x as f32,
                        y: image_rect.min.y + p.y as f32,
                    };
                    let size = Vec2::new(bb.w as f32, bb.h as f32);
                    let stroke =
                        Stroke::new(anno.outline.thickness, rgb_2_clr(Some(anno.outline.color)));
                    Shape::Rect(RectShape::new(
                        Rect::from_min_size(p, size),
                        Rounding::ZERO,
                        fill_rgb,
                        stroke,
                    ))
                })
            })
            .collect::<Vec<Shape>>();
        ui.painter().add(Shape::Vec(shapes));
    }
    fn collect_events(&mut self, ui: &mut Ui, image_response: &Response) -> rvlib::Events {
        let view_size = image_response.rect.size();
        let offset_x = image_response.rect.min.x;
        let offset_y = image_response.rect.min.y;
        let mouse_pos = image_response.hover_pos();
        let mouse_pos = mouse_pos.map(|mp| {
            view_pos_to_orig_pos(
                ((mp.x - offset_x), (mp.y - offset_y)).into(),
                self.shape_orig(),
                vec2_2_shape(view_size),
                &self.zoom_box,
            )
        });
        let key_events = map_key_events(ui);
        let mouse_events = map_mouse_events(image_response);

        rvlib::Events::default()
            .events(key_events)
            .events(mouse_events)
            .mousepos(mouse_pos)
    }

    fn add_image(&mut self, ui: &mut Ui) -> Option<Response> {
        self.texture.as_ref().map(|texture| {
            let ui_image = handle_2_image(
                texture,
                [self.shape_view().w as usize, self.shape_view().h as usize],
            )
            .shrink_to_fit()
            .sense(Sense::click_and_drag());

            ui.add(ui_image)
        })
    }
    fn update_texture(&mut self, ctx: &Context) {
        self.im_view = orig_2_view(&self.im_orig, self.zoom_box);
        self.texture = Some(clrim_2_handle(image_2_colorimage(&self.im_view), ctx));
    }
}

impl eframe::App for RvImageApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let update_view = self.event_loop.one_iteration(&self.events, ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Ok(update_view) = update_view {
                ui.label(&update_view.image_info);
                if let UpdateZoomBox::Yes(zb) = update_view.zoom_box {
                    self.zoom_box = zb;
                    self.update_texture(ctx);
                }
                if let UpdateImage::Yes(im) = update_view.image {
                    self.im_orig = im;
                    self.update_texture(ctx);
                }
                let image_response = self.add_image(ui);
                if let Some(ir) = image_response {
                    self.events = self.collect_events(ui, &ir);
                    if let UpdateAnnos::Yes((perm_annos, tmp_anno)) = update_view.annos {
                        self.annos = perm_annos;
                        if let Some(tmp_anno) = tmp_anno {
                            self.annos.push(tmp_anno);
                        }
                    }
                    if !self.annos.is_empty() {
                        self.draw_annos(ui, &ir.rect);
                    }
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
