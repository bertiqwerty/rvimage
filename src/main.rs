#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use image::imageops::FilterType;
use image::{imageops, ImageBuffer, Rgb};
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

    // application state in to create pixels buffer, i.e., everything not part of framework.gui()
    let mut image = ImageBuffer::<Rgb<u8>, _>::new(START_WIDTH, START_HEIGHT);
    let mut image_for_display = image.clone();
    let mut file_selected = None;
    let mut w_display = image.width();
    let mut h_display = image.height();
    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            let gui_file_selected = framework.gui().file_selected();
            if &file_selected != gui_file_selected {
                if let Some(path) = &gui_file_selected {
                    file_selected = gui_file_selected.clone();
                    let image_tmp = image::io::Reader::open(path).unwrap().decode().unwrap();
                    image = image_tmp.into_rgb8();
                    let resize_data = resize_to_surface(
                        &image,
                        window.inner_size().width,
                        window.inner_size().height,
                    );
                    match resize_data {
                        Some((im, w, h)) => {
                            image_for_display = im;
                            pixels.resize_buffer(w, h);
                            w_display = w;
                            h_display = h;
                        }
                        None => {
                            image_for_display = image.clone();
                            w_display = image.width();
                            h_display = image.height();
                            pixels.resize_buffer(image.width(), image.height());
                        }
                    }
                }
            }

            let mouse_pos = pixels
                .window_pos_to_pixel(match input.mouse() {
                    Some(pos) => pos,
                    None => (-1.0, -1.0),
                })
                .ok();

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                let resize_data = resize_to_surface(&image, size.width, size.height);
                match resize_data {
                    Some((im, w, h)) => {
                        image_for_display = im;
                        pixels.resize_buffer(w, h);
                        w_display = w;
                        h_display = h;
                    }
                    None => (),
                }
                pixels.resize_surface(size.width, size.height);
                framework.resize(size.width, size.height);
            }
            if framework.gui().file_selected().is_some() {
                let convert_coord = |x: usize, n_display: u32, n_total: u32| {
                    (x as f64 / n_display as f64 * n_total as f64) as usize
                };
                let pos_in_image = match mouse_pos {
                    Some((x, y)) if x < w_display as usize && y < h_display as usize => Some((
                        convert_coord(x, w_display, image.width()),
                        convert_coord(y, h_display, image.height()),
                    )),
                    _ => None,
                };
                framework.gui().set_state(
                    pos_in_image,
                    match pos_in_image {
                        Some((x, y)) => image.get_pixel(x as u32, y as u32).0,
                        _ => [0, 0, 0],
                    },
                    (image.width(), image.height()),
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
                draw(pixels.get_frame(), &image_for_display);

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

fn resize_to_surface(
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
fn draw(frame: &mut [u8], image: &ImageBuffer<Rgb<u8>, Vec<u8>>) {
    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
        let x = (i % image.width() as usize) as u32;
        let y = (i / image.width() as usize) as u32;
        let rgb = image.get_pixel(x, y).0;
        let rgba = [rgb[0], rgb[1], rgb[2], 0xff];

        pixel.copy_from_slice(&rgba);
    }
}
