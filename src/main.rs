#![deny(clippy::all)]
#![forbid(unsafe_code)]

use std::io;

use egui::{
    epaint::{CircleShape, PathShape, RectShape},
    Color32, ColorImage, Context, Image, Modifiers, PointerButton, Pos2, Rect, Response, Rounding,
    Sense, Shape, Stroke, Style, TextureHandle, TextureOptions, Ui, Vec2, Visuals,
};
use image::{ImageBuffer, Rgb};
use rvlib::{
    cfg::get_cfg_path,
    domain::{PtF, PtI},
    get_darkmode, orig_2_view, orig_pos_2_view_pos, project_on_bb, scale_coord,
    view_pos_2_orig_pos, Annotation, GeoFig, ImageU8, KeyCode, MainEventLoop, UpdateAnnos,
    UpdateImage, UpdateZoomBox, BB,
};
use tracing::{error, Level};
use tracing_subscriber::{
    fmt::{writer::MakeWriterExt, Layer},
    prelude::*,
};

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
        egui::Key::S => Some(rvlib::KeyCode::S),
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

#[derive(Default)]
struct LastSensedBtns {
    pub btn_codes: Vec<KeyCode>,
    pub modifiers: Vec<rvlib::Event>,
}
impl LastSensedBtns {
    fn is_empty(&self) -> bool {
        self.btn_codes.is_empty() && self.modifiers.is_empty()
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

fn rgb_2_clr(rgb: Option<[u8; 3]>, alpha: u8) -> Color32 {
    if let Some(rgb) = rgb {
        Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], alpha)
    } else {
        Color32::from_rgba_unmultiplied(0, 0, 0, 0)
    }
}

fn map_modifiers(modifiers: &Modifiers) -> Option<Vec<rvlib::Event>> {
    let mut events = Vec::new();
    if modifiers.alt {
        events.push(rvlib::Event::Held(KeyCode::Alt))
    }
    if modifiers.command {
        events.push(rvlib::Event::Held(KeyCode::Ctrl))
    }
    if modifiers.shift {
        events.push(rvlib::Event::Held(KeyCode::Shift))
    }
    Some(events)
}

fn map_key_events(ui: &mut Ui) -> Vec<rvlib::Event> {
    let mut events = vec![];
    ui.input(|i| {
        for e in i.events.iter() {
            if let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
            } = e
            {
                if let Some(k) = map_key(*key) {
                    if !pressed {
                        events.push(rvlib::Event::Released(k));
                    } else if !repeat {
                        events.push(rvlib::Event::Pressed(k));
                        events.push(rvlib::Event::Held(k));
                    } else {
                        events.push(rvlib::Event::Held(k));
                    }
                }
                let modifier_events = map_modifiers(modifiers);
                if let Some(mut me) = modifier_events {
                    events.append(&mut me);
                }
            }
        }
    });
    events
}

fn map_mouse_events(
    ui: &mut Ui,
    last_sensed: &mut LastSensedBtns,
    image_response: &Response,
) -> Vec<rvlib::Event> {
    let mut events = vec![];
    let mut btn_codes = LastSensedBtns::default();
    ui.input(|i| {
        for e in i.events.iter() {
            if let egui::Event::PointerButton {
                pos: _,
                button,
                pressed: _,
                modifiers,
            } = e
            {
                let modifier_events = map_modifiers(modifiers);
                if let Some(me) = modifier_events {
                    let btn_code = match button {
                        PointerButton::Primary => KeyCode::MouseLeft,
                        PointerButton::Secondary => KeyCode::MouseRight,
                        _ => KeyCode::DontCare,
                    };
                    btn_codes.btn_codes.push(btn_code);
                    btn_codes.modifiers = me;
                }
            }
        }
    });
    if !btn_codes.is_empty() {
        *last_sensed = btn_codes;
    }

    if image_response.clicked() || image_response.drag_released() {
        if last_sensed.btn_codes.contains(&KeyCode::MouseLeft) {
            events.push(rvlib::Event::Released(KeyCode::MouseLeft));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        } else if last_sensed.btn_codes.contains(&KeyCode::MouseRight) {
            events.push(rvlib::Event::Released(KeyCode::MouseRight));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        }
        *last_sensed = LastSensedBtns::default();
    }
    if image_response.drag_started() {
        if last_sensed.btn_codes.contains(&KeyCode::MouseLeft) {
            events.push(rvlib::Event::Pressed(KeyCode::MouseLeft));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        } else if last_sensed.btn_codes.contains(&KeyCode::MouseRight) {
            events.push(rvlib::Event::Pressed(KeyCode::MouseRight));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        }
    }
    if image_response.dragged() {
        if last_sensed.btn_codes.contains(&KeyCode::MouseLeft) {
            events.push(rvlib::Event::Held(KeyCode::MouseLeft));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        } else if last_sensed.btn_codes.contains(&KeyCode::MouseRight) {
            events.push(rvlib::Event::Held(KeyCode::MouseRight));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        }
    }
    events
}

fn vec2_2_shape(v: Vec2) -> rvlib::Shape {
    rvlib::Shape::new(v.x as u32, v.y as u32)
}

fn image_2_colorimage(im: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ColorImage {
    ColorImage::from_rgb([im.width() as usize, im.height() as usize], im.as_raw())
}

fn orig_pos_2_egui_rect(
    p: PtI,
    offset: Pos2,
    shape_orig: rvlib::Shape,
    shape_view: rvlib::Shape,
    rect_size: Vec2,
    zoom_box: &Option<BB>,
) -> Pos2 {
    let p = if let Some(zb) = zoom_box {
        project_on_bb(p, zb)
    } else {
        p
    };
    let p_view: PtF = orig_pos_2_view_pos(p, shape_orig, shape_view, zoom_box)
        .expect("After projection to zoombox it should be inside");
    let p_egui_rect_x = offset.x + scale_coord(p_view.x, shape_view.w as f32, rect_size.x);
    let p_egui_rect_y = offset.y + scale_coord(p_view.y, shape_view.h as f32, rect_size.y);
    Pos2::new(p_egui_rect_x, p_egui_rect_y)
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
    last_sensed_btncodes: LastSensedBtns,
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

    fn orig_pos_2_egui_rect(&self, p: PtI, offset: Pos2, rect_size: Vec2) -> Pos2 {
        orig_pos_2_egui_rect(
            p,
            offset,
            self.shape_orig(),
            self.shape_view(),
            rect_size,
            &self.zoom_box,
        )
    }
    fn draw_annos(&self, ui: &mut Ui, image_rect: &Rect) {
        let brush_annos = self
            .annos
            .iter()
            .flat_map(|anno| match anno {
                Annotation::Brush(brush) => Some(brush),
                _ => None,
            })
            .flat_map(|anno| {
                let size_from = self.shape_view().w as f32;
                let size_to = image_rect.size().x;
                let thickness = scale_coord(anno.outline.thickness, size_from, size_to);
                let stroke = Stroke::new(
                    thickness,
                    rgb_2_clr(
                        Some(anno.outline.color),
                        (anno.intensity.clamp(0.0, 1.0) * 255.0) as u8,
                    ),
                );
                let egui_rect_points = anno
                    .line
                    .points_iter()
                    .map(|p| self.orig_pos_2_egui_rect(p, image_rect.min, image_rect.size()))
                    .collect::<Vec<_>>();

                if egui_rect_points.len() > 3 {
                    Some(Shape::Path(PathShape::line(egui_rect_points, stroke)))
                } else {
                    let center = anno.line.mean();
                    if let Some(center) = center {
                        let center = self.orig_pos_2_egui_rect(
                            center.into(),
                            image_rect.min,
                            image_rect.size(),
                        );
                        Some(Shape::Circle(CircleShape::filled(
                            Pos2 {
                                x: center.x,
                                y: center.y,
                            },
                            stroke.width * 0.5,
                            stroke.color,
                        )))
                    } else {
                        None
                    }
                }
            });
        let bbox_annos = self
            .annos
            .iter()
            .flat_map(|anno| match anno {
                Annotation::Bbox(bbox) => Some(bbox),
                _ => None,
            })
            .map(|anno| {
                let (fill_alpha, outline_thickness) = if let Some(is_selected) = anno.is_selected {
                    if is_selected {
                        (
                            anno.fill_alpha.saturating_add(60),
                            anno.outline.thickness + 2.0,
                        )
                    } else {
                        (anno.fill_alpha, anno.outline.thickness)
                    }
                } else {
                    (anno.fill_alpha, anno.outline.thickness)
                };
                let fill_rgb = rgb_2_clr(anno.fill_color, fill_alpha);
                match &anno.geofig {
                    GeoFig::BB(bb) => {
                        let stroke = Stroke::new(
                            outline_thickness,
                            rgb_2_clr(Some(anno.outline.color), anno.outline_alpha),
                        );
                        let bb_min_rect =
                            self.orig_pos_2_egui_rect(bb.min(), image_rect.min, image_rect.size());
                        let bb_max_rect =
                            self.orig_pos_2_egui_rect(bb.max(), image_rect.min, image_rect.size());
                        Shape::Rect(RectShape::new(
                            Rect::from_min_max(bb_min_rect, bb_max_rect),
                            Rounding::ZERO,
                            fill_rgb,
                            stroke,
                        ))
                    }
                    GeoFig::Poly(poly) => {
                        let stroke = Stroke::new(
                            outline_thickness,
                            rgb_2_clr(Some(anno.outline.color), anno.outline_alpha),
                        );
                        let poly = if let Some(zb) = self.zoom_box {
                            if let Ok(poly_) = poly.clone().intersect(zb) {
                                poly_
                            } else {
                                poly.clone()
                            }
                        } else {
                            poly.clone()
                        };
                        let egui_rect_points = poly
                            .points_iter()
                            .map(|p| {
                                self.orig_pos_2_egui_rect(p, image_rect.min, image_rect.size())
                            })
                            .collect::<Vec<_>>();

                        Shape::Path(PathShape::closed_line(egui_rect_points, stroke))
                    }
                }
            });
        let shapes = brush_annos.chain(bbox_annos).collect::<Vec<Shape>>();
        ui.painter().add(Shape::Vec(shapes));
    }
    fn collect_events(&mut self, ui: &mut Ui, image_response: &Response) -> rvlib::Events {
        let rect_size = image_response.rect.size();
        let offset_x = image_response.rect.min.x;
        let offset_y = image_response.rect.min.y;
        let mouse_pos = image_response.hover_pos();
        let mouse_pos = mouse_pos.map(|mp| {
            let view_pos = view_pos_2_orig_pos(
                ((mp.x - offset_x), (mp.y - offset_y)).into(),
                self.shape_view(),
                vec2_2_shape(rect_size),
                &None,
            );
            view_pos_2_orig_pos(
                view_pos,
                self.shape_orig(),
                self.shape_view(),
                &self.zoom_box,
            )
        });
        let key_events = map_key_events(ui);
        let mouse_events = map_mouse_events(ui, &mut self.last_sensed_btncodes, image_response);

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
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
        });
        let update_view = self.event_loop.one_iteration(&self.events, ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Ok(update_view) = update_view {
                if let UpdateZoomBox::Yes(zb) = update_view.zoom_box {
                    self.zoom_box = zb;
                    self.update_texture(ctx);
                }
                if let UpdateImage::Yes(im) = update_view.image {
                    self.im_orig = im;
                    self.update_texture(ctx);
                }

                if let Some(info) = update_view.image_info {
                    ui.label(format!(
                        "{}  |  {}  |  {}",
                        info.filename, info.shape_info, info.pixel_value
                    ));
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
                    };
                }
            }
        });
    }
}

fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    let cfg_path = get_cfg_path().unwrap();
    let log_folder = cfg_path.parent().unwrap();
    let file_appender = tracing_appender::rolling::daily(log_folder, "rvimage.log");
    let (file_appender, _guard_file) = tracing_appender::non_blocking(file_appender);
    let file_appender = Layer::new()
        .with_writer(file_appender.with_max_level(Level::INFO))
        .with_line_number(true)
        .with_file(true);

    let stdout = Layer::new()
        .with_writer(io::stdout.with_max_level(Level::INFO));
    tracing_subscriber::registry()
        .with(file_appender)
        .with(stdout)
        .init();

    let native_options = eframe::NativeOptions::default();
    if let Err(e) = eframe::run_native(
        "RV Image",
        native_options,
        Box::new(|cc| {
            if let Some(dm) = get_darkmode() {
                let viz = if dm {
                    Visuals::dark()
                } else {
                    Visuals::light()
                };
                let style = Style {
                    visuals: viz,
                    ..Style::default()
                };
                cc.egui_ctx.set_style(style);
            }
            Box::new(RvImageApp::new(cc))
        }),
    ) {
        error!("{e:?}");
    }
}
