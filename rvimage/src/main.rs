#![deny(clippy::all)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![forbid(unsafe_code)]

use clap::Parser;

use egui::{
    epaint::{CircleShape, PathShape, RectShape},
    Color32, ColorImage, Context, Image, Modifiers, PointerButton, Pos2, Rect, Response, Rounding,
    Sense, Shape, Stroke, Style, TextureHandle, TextureOptions, Ui, Vec2, ViewportCommand, Visuals,
};
use image::{GenericImage, ImageBuffer, Rgb};
use imageproc::distance_transform::Norm;
use rvimage_domain::{access_mask_abs, to_rv, BbF, Canvas, PtF, PtI, RvResult, ShapeF, TPtF, TPtI};
use rvlib::{
    cfg::{ExportPath, ExportPathConnection},
    color_with_intensity,
    control::Control,
    file_util::osstr_to_str,
    read_darkmode,
    result::trace_ok_err,
    to_per_file_crowd,
    tools::{self, BBOX_NAME, BRUSH_NAME},
    tracing_setup,
    view::{self, ImageU8},
    write_coco, Annotation, BboxAnnotation, BrushAnnotation, GeoFig, InstanceAnnotate, KeyCode,
    MainEventLoop, MetaData, Rot90ToolData, ShapeI, UpdateImage, UpdatePermAnnos, UpdateTmpAnno,
    UpdateZoomBox, ZoomAmount,
};
use std::{iter, ops::Deref, panic, path::Path, sync::Arc, time::Instant};
use tracing::error;

fn map_key(egui_key: egui::Key) -> Option<rvlib::KeyCode> {
    match egui_key {
        egui::Key::A => Some(rvlib::KeyCode::A),
        egui::Key::B => Some(rvlib::KeyCode::B),
        egui::Key::C => Some(rvlib::KeyCode::C),
        egui::Key::D => Some(rvlib::KeyCode::D),
        egui::Key::E => Some(rvlib::KeyCode::E),
        egui::Key::L => Some(rvlib::KeyCode::L),
        egui::Key::H => Some(rvlib::KeyCode::H),
        egui::Key::I => Some(rvlib::KeyCode::I),
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
        egui::Key::Plus | egui::Key::Equals => Some(rvlib::KeyCode::PlusEquals),
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

fn setup_custom_fonts(ctx: &egui::Context) {
    // Start with the default fonts (we will be adding to them rather than replacing them).
    let mut fonts = egui::FontDefinitions::default();

    // Install my own font (maybe supporting non-latin characters).
    // .ttf and .otf files supported.
    fonts.font_data.insert(
        "roboto_mono".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../resources/Roboto/RobotoMono-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "roboto".to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../resources/Roboto/Roboto-Regular.ttf"
        ))),
    );

    // Put my font first (highest priority) for proportional text:
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "roboto".to_owned());

    // Put my font as last fallback for monospace:
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(1, "roboto_mono".to_owned());

    // Tell egui to use these fonts:
    ctx.set_fonts(fonts);
}

#[derive(Debug, Default)]
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

fn map_modifiers(modifiers: Modifiers) -> Vec<rvlib::Event> {
    let mut events = Vec::new();
    if modifiers.alt {
        events.push(rvlib::Event::Held(KeyCode::Alt));
    }
    if modifiers.command || modifiers.ctrl {
        events.push(rvlib::Event::Held(KeyCode::Ctrl));
    }
    if modifiers.shift {
        events.push(rvlib::Event::Held(KeyCode::Shift));
    }
    events
}

fn map_key_events(ui: &mut Ui) -> Vec<rvlib::Event> {
    let mut events = vec![];
    ui.input(|i| {
        for e in &i.events {
            if let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                physical_key: _,
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
                let mut modifier_events = map_modifiers(*modifiers);
                events.append(&mut modifier_events);
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
        for e in &i.events {
            match e {
                egui::Event::PointerButton {
                    pos: _,
                    button,
                    pressed: _,
                    modifiers,
                } => {
                    let modifier_events = map_modifiers(*modifiers);
                    let btn_code = match button {
                        PointerButton::Primary => KeyCode::MouseLeft,
                        PointerButton::Secondary => KeyCode::MouseRight,
                        _ => KeyCode::DontCare,
                    };
                    btn_codes.btn_codes.push(btn_code);
                    btn_codes.modifiers = modifier_events;
                }
                egui::Event::Zoom(z) => {
                    events.push(rvlib::Event::Zoom(ZoomAmount::Factor(f64::from(*z))));
                }
                egui::Event::MouseWheel {
                    unit: _,
                    delta,
                    modifiers,
                } => {
                    if modifiers.ctrl {
                        events.push(rvlib::Event::Zoom(ZoomAmount::Delta(f64::from(delta.y))));
                    }
                }
                _ => {}
            };
        }
    });
    if !btn_codes.is_empty() {
        *last_sensed = btn_codes;
    }

    if image_response.clicked()
        || image_response.secondary_clicked()
        || image_response.drag_stopped()
    {
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

fn vec2_2_shape(v: Vec2) -> rvlib::ShapeI {
    rvlib::ShapeI::new(v.x as u32, v.y as u32)
}

fn image_2_colorimage(im: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ColorImage {
    ColorImage::from_rgb([im.width() as usize, im.height() as usize], im.as_raw())
}

fn orig_pos_2_egui_rect(
    p: PtF,
    offset: Pos2,
    shape_orig: rvlib::ShapeI,
    shape_view: rvlib::ShapeI,
    rect_size: Vec2,
    zoom_box: &Option<BbF>,
) -> Pos2 {
    let p = if let Some(zb) = zoom_box {
        view::project_on_bb(p, zb)
    } else {
        p
    };
    let p_view: PtF = view::pos_from_orig_pos(p, shape_orig, shape_view, zoom_box)
        .expect("After projection to zoombox it should be inside");
    let p_egui_rect_x =
        offset.x + view::scale_coord(p_view.x, shape_view.w.into(), TPtF::from(rect_size.x)) as f32;
    let p_egui_rect_y =
        offset.y + view::scale_coord(p_view.y, shape_view.h.into(), TPtF::from(rect_size.y)) as f32;
    Pos2::new(p_egui_rect_x, p_egui_rect_y)
}

fn color_tf(intensity: TPtF, color: [u8; 3], alpha: u8) -> (Rgb<u8>, Color32) {
    let min_instensity = 0.3;
    let max_instensity = 1.0;
    let intensity_span = max_instensity - min_instensity;
    let viz_intensity = intensity * intensity_span + min_instensity;
    let color_rgb = color_with_intensity(Rgb(color), viz_intensity);
    let color_egui = rgb_2_clr(Some(color_rgb.0), alpha);
    (color_rgb, color_egui)
}

#[derive(Default)]
struct RvImageApp {
    event_loop: MainEventLoop,
    texture: Option<TextureHandle>,
    annos: Vec<Annotation>,
    zoom_box: Option<BbF>,
    im_orig: ImageU8,
    im_view: ImageU8,
    events: rvlib::Events,
    last_sensed_btncodes: LastSensedBtns,
    t_last_iterations: [f64; 3],
    egui_perm_shapes: Vec<Shape>,
    egui_tmp_shapes: [Option<Shape>; 2],
    image_rect: Option<Rect>,
}

impl RvImageApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Install my own font (maybe supporting non-latin characters).
        // .ttf and .otf files supported.
        setup_custom_fonts(&cc.egui_ctx);
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self::default()
    }
    fn shape_orig(&self) -> rvlib::ShapeI {
        rvlib::ShapeI::from_im(&self.im_orig)
    }
    fn shape_view(&self) -> rvlib::ShapeI {
        rvlib::ShapeI::from_im(&self.im_view)
    }

    fn orig_pos_2_egui_rect(&self, p: PtF, offset: Pos2, rect_size: Vec2) -> Pos2 {
        orig_pos_2_egui_rect(
            p,
            offset,
            self.shape_orig(),
            self.shape_view(),
            rect_size,
            &self.zoom_box,
        )
    }
    fn update_brush_anno_tmp(
        &self,
        anno: &BrushAnnotation,
        image_rect: &Rect,
    ) -> Option<egui::epaint::Shape> {
        let size_from = self.shape_view().w.into();
        let size_to = TPtF::from(image_rect.size().x);
        let (_, color_egui) = color_tf(anno.canvas.intensity, anno.color, anno.fill_alpha);
        if let Some(tmp_line) = &anno.tmp_line {
            let make_shape_vec = |thickness, color| {
                let egui_rect_points = tmp_line
                    .line
                    .points_iter()
                    .map(|p| self.orig_pos_2_egui_rect(p, image_rect.min, image_rect.size()))
                    .collect::<Vec<_>>();
                let stroke = Stroke::new(thickness as f32, color);
                let start_circle = egui_rect_points
                    .first()
                    .map(|p| Shape::Circle(CircleShape::filled(*p, thickness as f32 * 0.5, color)));
                let end_circle = egui_rect_points
                    .last()
                    .map(|p| Shape::Circle(CircleShape::filled(*p, thickness as f32 * 0.5, color)));
                let end_circle = if egui_rect_points.len() > 1 {
                    end_circle
                } else {
                    None
                };
                let line = if egui_rect_points.len() > 2 {
                    Some(Shape::Path(PathShape::line(egui_rect_points, stroke)))
                } else {
                    None
                };
                [start_circle, line, end_circle]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            };
            let thickness = view::scale_coord(tmp_line.thickness, size_from, size_to);
            let mut shape_vec = make_shape_vec(thickness, color_egui);
            let mut selected_shape_vec = if anno.is_selected == Some(true) {
                make_shape_vec(thickness + 5.0, rgb_2_clr(Some([0, 0, 0]), 255))
            } else {
                vec![]
            };
            selected_shape_vec.append(&mut shape_vec);
            Some(Shape::Vec(selected_shape_vec))
        } else {
            None
        }
    }
    fn update_bbox_anno(&self, anno: &BboxAnnotation, image_rect: &Rect) -> egui::epaint::Shape {
        let (fill_alpha, outline_thickness) = if anno.is_selected == Some(true) {
            (
                anno.fill_alpha.saturating_add(60),
                anno.outline.thickness + 2.0,
            )
        } else {
            (anno.fill_alpha, anno.outline.thickness)
        };
        let fill_rgb = rgb_2_clr(anno.fill_color, fill_alpha);
        let mut draw_vec = anno
            .highlight_circles
            .iter()
            .map(|c| {
                let p = self.orig_pos_2_egui_rect(c.center, image_rect.min, image_rect.size());
                Shape::Circle(CircleShape::filled(p, c.radius as f32, Color32::WHITE))
            })
            .collect::<Vec<_>>();
        match &anno.geofig {
            GeoFig::BB(bb) => {
                let stroke = Stroke::new(
                    outline_thickness as f32,
                    rgb_2_clr(Some(anno.outline.color), anno.outline_alpha),
                );
                let bb_min_rect =
                    self.orig_pos_2_egui_rect(bb.min(), image_rect.min, image_rect.size());
                let bb_max_rect =
                    self.orig_pos_2_egui_rect(bb.max(), image_rect.min, image_rect.size());
                draw_vec.push(Shape::Rect(RectShape::new(
                    Rect::from_min_max(bb_min_rect, bb_max_rect),
                    Rounding::ZERO,
                    fill_rgb,
                    stroke,
                )));
            }
            GeoFig::Poly(poly) => {
                let stroke = Stroke::new(
                    outline_thickness as f32,
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
                    .map(|p| self.orig_pos_2_egui_rect(p, image_rect.min, image_rect.size()))
                    .collect::<Vec<_>>();
                draw_vec.push(Shape::Path(PathShape::closed_line(
                    egui_rect_points,
                    stroke,
                )));
            }
        }
        Shape::Vec(draw_vec)
    }
    fn update_perm_annos(&mut self, image_rect: &Rect) {
        let canvases = self
            .annos
            .iter()
            .filter_map(|anno| match anno {
                Annotation::Brush(brush) => Some(brush),
                Annotation::Bbox(_) => None,
            })
            .flat_map(|anno| {
                let (color_rgb, _) = color_tf(anno.canvas.intensity, anno.color, 255);
                let mut res = [None, None];
                if anno.is_selected == Some(true) {
                    let mask = ImageBuffer::from_vec(
                        anno.canvas.bb.w,
                        anno.canvas.bb.h,
                        anno.canvas.mask.clone(),
                    );

                    if let Some(mask) = mask {
                        let k = 5u8;
                        let expansion = TPtI::from(k / 2);
                        let new_bb = anno
                            .canvas
                            .bb
                            .expand(expansion, expansion, self.shape_orig());
                        let mut selection_viz = ImageBuffer::new(new_bb.w, new_bb.h);
                        trace_ok_err(selection_viz.copy_from(&mask, expansion, expansion));
                        let selection_viz =
                            imageproc::morphology::dilate(&selection_viz, Norm::L1, k);
                        let selection_viz_canvas = Canvas {
                            mask: selection_viz.to_vec(),
                            bb: new_bb,
                            intensity: 1.0,
                        };
                        res[0] = Some((selection_viz_canvas, (Rgb([0, 0, 0]), 255)));
                    }
                }
                res[1] = Some((anno.canvas.clone(), (color_rgb, anno.fill_alpha)));
                res
            });
        let bbox_annos = self
            .annos
            .iter()
            .filter_map(|anno| match anno {
                Annotation::Bbox(bbox) => {
                    // hide out of zoombox geos
                    if self.zoom_box.map(|zb| {
                        (0..3)
                            .map(|corner_idx| bbox.geofig.enclosing_bb().corner(corner_idx))
                            .chain(iter::once(bbox.geofig.enclosing_bb().center_f().into()))
                            .any(|p| zb.contains(p))
                    }) == Some(false)
                    {
                        None
                    } else {
                        Some(bbox)
                    }
                }
                Annotation::Brush(_) => None,
            })
            .map(|anno| self.update_bbox_anno(anno, image_rect));
        // update texture with brush canvas
        let shape_orig = self.shape_orig();
        let mut im_view = view::from_orig(&self.im_orig, self.zoom_box);
        for (canvas, (color, fill_alpha)) in canvases.flatten() {
            for y in canvas.bb.y_range() {
                for x in canvas.bb.x_range() {
                    let p = PtI { x, y };
                    let is_fg = access_mask_abs(&canvas.mask, canvas.bb, p) > 0;
                    if is_fg {
                        let p_view = view::pos_from_orig_pos(
                            p.into(),
                            shape_orig,
                            ShapeI::from_im(&im_view),
                            &self.zoom_box,
                        );
                        if let Some(p_view) = p_view {
                            let (x_view, y_view) = (p_view.x as u32, p_view.y as u32);
                            let current_clr = im_view.get_pixel_checked(x_view, y_view);
                            if let Some(current_clr) = current_clr {
                                let alpha = f32::from(fill_alpha) / 255.0;
                                let mut clr = color.0;
                                for i in 0..3 {
                                    clr[i] = (f32::from(clr[i]) * alpha
                                        + f32::from(current_clr[i]) * (1.0 - alpha))
                                        .round()
                                        .clamp(0.0, 255.0)
                                        as u8;
                                }
                                im_view.put_pixel(x_view, y_view, Rgb(clr));
                            }
                        }
                    }
                }
            }
        }
        self.egui_perm_shapes = bbox_annos.collect::<Vec<Shape>>();
        self.im_view = im_view;
    }
    fn draw_annos(&mut self, ui: &mut Ui, update_texture: bool) {
        if let Some(texture) = self.texture.as_mut() {
            if update_texture {
                let im = image_2_colorimage(&self.im_view);
                texture.set(im, TextureOptions::NEAREST);
            }
        }
        ui.painter().add(Shape::Vec(self.egui_perm_shapes.clone()));
        ui.painter().add(Shape::Vec(
            self.egui_tmp_shapes
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>(),
        ));
    }
    fn collect_events(&mut self, ui: &mut Ui, image_response: &Response) -> rvlib::Events {
        let rect_size = image_response.rect.size();
        let offset_x = image_response.rect.min.x;
        let offset_y = image_response.rect.min.y;
        let mouse_pos_egui = image_response.hover_pos();
        let mouse_pos_on_view = mouse_pos_egui.map(|mp_egui| {
            view::pos_2_orig_pos(
                (
                    TPtF::from(mp_egui.x - offset_x),
                    TPtF::from(mp_egui.y - offset_y),
                )
                    .into(),
                self.shape_view(),
                vec2_2_shape(rect_size),
                &None,
            )
        });
        let mouse_pos = mouse_pos_on_view.map(|mp_view| {
            view::pos_2_orig_pos(
                mp_view,
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
            .mousepos_orig(mouse_pos)
            .mousepos_view(mouse_pos_on_view)
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
        self.im_view = view::from_orig(&self.im_orig, self.zoom_box);
        self.texture = Some(clrim_2_handle(image_2_colorimage(&self.im_view), ctx));
    }
}

impl eframe::App for RvImageApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        let start = Instant::now();
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = false;
        });
        let res = self.event_loop.one_iteration(
            &self.events,
            self.image_rect
                .map(|ir| ShapeF::new(ir.width().into(), ir.height().into())),
            ctx,
        );
        if let Ok((update_view, prj_name)) = res {
            let title = if prj_name.is_empty() {
                "RV Image".to_string()
            } else {
                format!("RV Image - {prj_name}")
            };
            ctx.send_viewport_cmd(ViewportCommand::Title(title));
            egui::CentralPanel::default().show(ctx, |ui| {
                if let UpdateZoomBox::Yes(zb) = update_view.zoom_box {
                    self.zoom_box = zb;
                    self.update_texture(ctx);
                }
                if let UpdateImage::Yes(im) = update_view.image {
                    self.im_orig = im;
                    self.update_texture(ctx);
                }
                let it_per_s = 1.0
                    / (self.t_last_iterations.iter().sum::<f64>()
                        / self.t_last_iterations.len() as f64);
                let it_str = if it_per_s > 1000.0 {
                    "1000+".to_string()
                } else {
                    format!("{}", it_per_s.round())
                };
                if let Some(info) = update_view.image_info {
                    ui.add(
                        egui::Label::new(egui::RichText::new(info.filename).monospace()).truncate(),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "{} | {} | {} it/s",
                                info.pixel_value, info.shape_info, it_str
                            ))
                            .monospace(),
                        )
                        .truncate(),
                    );
                    let image_surrounding_rect = ui.max_rect();
                    let image_response = self.add_image(ui);
                    let mut update_texture = false;
                    if let Some(ir) = image_response {
                        // react to resizing the image rect
                        if self.image_rect != Some(ir.rect) {
                            self.update_perm_annos(&ir.rect);

                            self.image_rect = Some(image_surrounding_rect);
                        }
                        self.events = self.collect_events(ui, &ir);
                        if let UpdatePermAnnos::Yes(perm_annos) = update_view.perm_annos {
                            self.annos = perm_annos;
                            self.update_perm_annos(&ir.rect);
                            update_texture = true;
                        }
                        match update_view.tmp_annos {
                            UpdateTmpAnno::Yes(anno) => {
                                self.egui_tmp_shapes = [None, None];
                                match anno {
                                    Annotation::Brush(brush) => {
                                        if let Some(shape) =
                                            self.update_brush_anno_tmp(&brush, &ir.rect)
                                        {
                                            self.egui_tmp_shapes[0] = Some(shape);
                                        }
                                    }
                                    Annotation::Bbox(bbox) => {
                                        self.egui_tmp_shapes[1] =
                                            Some(self.update_bbox_anno(&bbox, &ir.rect));
                                    }
                                }
                            }
                            UpdateTmpAnno::No => {
                                self.egui_tmp_shapes = [None, None];
                            }
                        }
                        self.draw_annos(ui, update_texture);
                    };
                }
            });
        }
        let n_millis = self.t_last_iterations.len();
        for i in 0..(n_millis - 1) {
            self.t_last_iterations[i] = self.t_last_iterations[i + 1];
        }
        self.t_last_iterations[n_millis - 1] = start.elapsed().as_secs_f64();
    }
}

#[derive(Parser)]
struct Cli {
    in_prj_path: Option<std::path::PathBuf>,
    out_folder: Option<std::path::PathBuf>,
    #[arg(short, long)]
    per_file_crowd: bool,
}
fn export_coco_path(
    in_prj_path: &Path,
    out_folder: &Path,
    name: &'static str,
) -> RvResult<ExportPath> {
    Ok(ExportPath {
        path: out_folder.join(format!(
            "{}_coco_{name}.json",
            osstr_to_str(in_prj_path.file_stem()).map_err(to_rv)?
        )),
        conn: ExportPathConnection::Local,
    })
}
fn export_coco(
    in_prj_path: &Path,
    out_folder: &Path,
    per_file_crowd: bool,
) -> RvResult<(Vec<ExportPath>, MetaData, Option<Rot90ToolData>)> {
    let mut ctrl = Control::default();
    let tdm = ctrl.load(in_prj_path.to_path_buf())?;
    let meta_data = ctrl.meta_data(None, None);
    let rot90 = tdm
        .get(tools::ROT90_NAME)
        .and_then(|d| d.specifics.rot90().ok());
    let mut handles = vec![];
    let mut export_paths = vec![];
    if let Some(tools_data) = tdm.get(BBOX_NAME) {
        let export_path = export_coco_path(in_prj_path, out_folder, BBOX_NAME)?;
        tracing::info!("{:?}", export_path.path);
        let (_, handle) = write_coco(
            &meta_data,
            tools_data.specifics.bbox()?.clone(),
            rot90,
            &export_path,
        )?;
        export_paths.push(export_path);
        handles.push(handle);
    }
    if let Some(tools_data) = tdm.get(BRUSH_NAME) {
        let export_path = export_coco_path(in_prj_path, out_folder, BRUSH_NAME)?;
        tracing::info!("{:?}", export_path.path);
        let mut brush_data = tools_data.specifics.brush()?.clone();
        if per_file_crowd {
            to_per_file_crowd(&mut brush_data.annotations_map);
        }
        let (_, handle) = write_coco(&meta_data, brush_data, rot90, &export_path)?;
        handles.push(handle);
        export_paths.push(export_path);
    }
    for handle in handles {
        handle.join().map_err(to_rv)??;
    }
    Ok((export_paths, meta_data, rot90.cloned()))
}
fn main() {
    let _guard_flush_to_logfile = tracing_setup::tracing_setup();
    if let Err(e) = panic::catch_unwind(|| {
        let cli = Cli::parse();
        if let (Some(in_prj_path), Some(out_folder)) = (cli.in_prj_path, cli.out_folder) {
            trace_ok_err(export_coco(&in_prj_path, &out_folder, cli.per_file_crowd));
        } else {
            let native_options = eframe::NativeOptions::default();
            if let Err(e) = eframe::run_native(
                "RV Image",
                native_options,
                Box::new(|cc| {
                    if let Some(dm) = read_darkmode() {
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
                    Ok(Box::new(RvImageApp::new(cc)))
                }),
            ) {
                error!("{e:?}");
            }
        }
    }) {
        let panic_s = e
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| e.downcast_ref::<&'static str>().map(Deref::deref));
        tracing::error!("{:?}", panic_s);
        let b = tracing_setup::BACKTRACE
            .with(|b| b.borrow_mut().take())
            .unwrap();
        tracing::error!("{:?}", b);
    }
}

#[cfg(test)]
use {
    rvlib::{defer_file_removal, defer_folder_removal, file_util::DEFAULT_TMPDIR, read_coco},
    std::fs,
    std::path::PathBuf,
};

#[test]
fn test_coco() {
    let in_prj_path = PathBuf::from("resources/test_data/rvprj_v4-0.json");
    let test_file = in_prj_path.parent().unwrap().join("tmp-test.rvi");
    defer_file_removal!(&test_file);
    fs::copy(&in_prj_path, &test_file).unwrap();
    let tmp_folder = DEFAULT_TMPDIR.join("convertcocotest");
    std::fs::create_dir_all(&tmp_folder).unwrap();
    defer_folder_removal!(&tmp_folder);
    let (export_path, meta_data, rot90) = export_coco(&test_file, &tmp_folder, true).unwrap();
    let files = tmp_folder
        .read_dir()
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();

            entry.path()
        })
        .collect::<Vec<_>>();
    files
        .iter()
        .find(|f| osstr_to_str(f.file_stem()).unwrap().contains(BBOX_NAME))
        .unwrap();
    files
        .iter()
        .find(|f| osstr_to_str(f.file_stem()).unwrap().contains(BRUSH_NAME))
        .unwrap();
    for ep in export_path {
        read_coco(&meta_data, &ep, rot90.as_ref()).unwrap();
    }
}
