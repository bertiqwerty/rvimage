#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use image::imageops::FilterType;
use image::{imageops, GenericImageView, ImageBuffer, Rgb, SubImage};
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod gui;

const START_WIDTH: u32 = 512;
const START_HEIGHT: u32 = 512;

const LEFT_BTN: usize = 0;

const MIN_CROP: usize = 10;

#[derive(Clone, Copy)]
struct Crop {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

struct BufferContent {
    image_transformed: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

impl BufferContent {
    pub fn new(im: ImageBuffer<Rgb<u8>, Vec<u8>>) -> Self {
        Self {
            image_transformed: im,
        }
    }
    pub fn view<'a>(
        &'a self,
        crop: &Option<(&'a ImageBuffer<Rgb<u8>, Vec<u8>>, Crop)>,
    ) -> SubImage<&'a ImageBuffer<Rgb<u8>, Vec<u8>>> {
        match crop {
            Some((im_o, crop)) => {
                im_o.view(crop.x as u32, crop.y as u32, crop.w as u32, crop.h as u32)
            }
            None => self.image_transformed.view(
                0,
                0,
                self.image_transformed.width(),
                self.image_transformed.height(),
            ),
        }
    }
}

fn coord_trans_2_orig(x: usize, n_transformed: u32, n_orig: u32) -> usize {
    (x as f64 / n_transformed as f64 * n_orig as f64) as usize
}

fn main() -> Result<(), Error> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(START_WIDTH as f64, START_HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Rimview")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(START_WIDTH, START_HEIGHT, surface_texture)?;
        let framework =
            Framework::new(window_size.width, window_size.height, scale_factor, &pixels);
        (pixels, framework)
    };

    // application state to create pixels buffer, i.e., everything not part of framework.gui()
    let mut image_orig = ImageBuffer::<Rgb<u8>, _>::new(START_WIDTH, START_HEIGHT);
    let mut buffer_content = BufferContent::new(image_orig.clone());
    let mut crop: Option<Crop> = None;
    let mut crop_start: Option<(usize, usize)> = None;
    let mut file_selected = None;
    let mut w_display = image_orig.width();
    let mut h_display = image_orig.height();

    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            let mouse_pos = pixels
                .window_pos_to_pixel(match input.mouse() {
                    Some(pos) => pos,
                    None => (-1.0, -1.0),
                })
                .ok();

            // crop
            if input.mouse_pressed(LEFT_BTN) {
                if crop_start.is_none() {
                    if let Some((x, y)) = mouse_pos {
                        crop_start = Some((x, y));
                    }
                }
            }
            if input.mouse_released(LEFT_BTN) {
                match (crop_start, mouse_pos) {
                    (Some((c_x, c_y)), Some((m_x, m_y))) => {
                        let x_min = c_x.min(m_x);
                        let y_min = c_y.min(m_y);
                        let x_max = c_x.max(m_x);
                        let y_max = c_y.max(m_y);
                        let w = x_max - x_min;
                        let h = y_max - y_min;
                        if w > MIN_CROP && h > MIN_CROP {
                            let w_transformed = buffer_content.image_transformed.width();
                            let h_transformed = buffer_content.image_transformed.height();
                            let w_orig = image_orig.width();
                            let h_orig = image_orig.height();
                            crop = Some(Crop {
                                x: coord_trans_2_orig(x_min, w_transformed, w_orig),
                                y: coord_trans_2_orig(y_min, h_transformed, h_orig),
                                w: coord_trans_2_orig(w, w_transformed, w_orig),
                                h: coord_trans_2_orig(h, h_transformed, h_orig),
                            });
                            pixels.resize_buffer(w as u32, h as u32);
                        }
                        crop_start = None;
                    }
                    _ => (),
                }
            }

            // load new image
            let gui_file_selected = framework.gui().file_selected();
            if &file_selected != gui_file_selected {
                if let Some(path) = &gui_file_selected {
                    file_selected = gui_file_selected.clone();
                    let image_tmp = image::io::Reader::open(path).unwrap().decode().unwrap();
                    image_orig = image_tmp.into_rgb8();
                    let resize_data = resize_image_to_surface(
                        &image_orig,
                        window.inner_size().width,
                        window.inner_size().height,
                    );
                    match resize_data {
                        Some((im, w, h)) => {
                            buffer_content.image_transformed = im;
                            pixels.resize_buffer(w, h);
                            w_display = w;
                            h_display = h;
                        }
                        None => {
                            buffer_content.image_transformed = image_orig.clone();
                            w_display = image_orig.width();
                            h_display = image_orig.height();
                            pixels.resize_buffer(image_orig.width(), image_orig.height());
                        }
                    }
                }
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                let resize_data = resize_image_to_surface(&image_orig, size.width, size.height);
                match resize_data {
                    Some((im, w, h)) => {
                        buffer_content = BufferContent::new(im);
                        pixels.resize_buffer(w, h);
                        w_display = w;
                        h_display = h;
                    }
                    None => (),
                }
                pixels.resize_surface(size.width, size.height);
                framework.resize(size.width, size.height);
            }

            // show position and rgb value
            if framework.gui().file_selected().is_some() {
                let (x_off, y_off) = match &crop {
                    Some(c) => (c.x, c.y),
                    _ => (0, 0),
                };

                let pos_in_image = match mouse_pos {
                    Some((x, y)) if x < w_display as usize && y < h_display as usize => Some((
                        x_off + coord_trans_2_orig(x, w_display, image_orig.width()),
                        y_off + coord_trans_2_orig(y, h_display, image_orig.height()),
                    )),
                    _ => None,
                };
                framework.gui().set_state(
                    pos_in_image,
                    match pos_in_image {
                        Some((x, y)) => image_orig.get_pixel(x as u32, y as u32).0,
                        _ => [0, 0, 0],
                    },
                    (image_orig.width(), image_orig.height()),
                );
            } else {
                framework.gui().set_state(None, [0, 0, 0], (0, 0));
            }

            window.request_redraw();
        }

        match event {
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                framework.handle_event(&event);
            }
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // Draw the world
                draw(pixels.get_frame(), &buffer_content, &crop.map(|c| (&image_orig, c.clone())));

                // Prepare egui
                framework.prepare(&window);

                // Render everything together
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // Render the world texture
                    context.scaling_renderer.render(encoder, render_target);

                    // Render egui
                    framework.render(encoder, render_target, context)?;

                    Ok(())
                });

                // Basic error handling
                if render_result
                    .map_err(|e| error!("pixels.render() failed: {}", e))
                    .is_err()
                {
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => (),
        }
    });
}

fn resize_image_to_surface(
    image: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    surf_w: u32,
    surf_h: u32,
) -> Option<(ImageBuffer<Rgb<u8>, Vec<u8>>, u32, u32)> {
    if image.width() > surf_w || image.height() > surf_h {
        let w_ratio = image.width() as f64 / surf_w as f64;
        let h_ratio = image.height() as f64 / surf_h as f64;
        let ratio = w_ratio.max(h_ratio);
        let w_new = (image.width() as f64 / ratio) as u32;
        let h_new = (image.height() as f64 / ratio) as u32;
        let im_resized = imageops::resize(image, w_new, h_new, FilterType::Nearest);
        Some((im_resized, w_new, h_new))
    } else {
        None
    }
}

/// Draw the image to the frame buffer.
///
/// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
fn draw(frame: &mut [u8], buffer_content: &BufferContent, crop: &Option<(&ImageBuffer<Rgb<u8>, Vec<u8>>, Crop)>) {
    let sub_image = buffer_content.view(crop);

    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
        let x = (i % sub_image.width() as usize) as u32;
        let y = (i / sub_image.width() as usize) as u32;
        let rgb = sub_image.get_pixel(x, y).0;
        let rgba = [rgb[0], rgb[1], rgb[2], 0xff];

        pixel.copy_from_slice(&rgba);
    }
}
