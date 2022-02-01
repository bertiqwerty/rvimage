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

/// Everything we need to draw
struct World {
    im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>,
    im_transformed: ImageBuffer<Rgb<u8>, Vec<u8>>,
    crop: Option<Crop>,
}

impl World {
    pub fn new(im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>) -> Self {
        Self {
            im_orig: im_orig.clone(),
            im_transformed: im_orig,
            crop: None,
        }
    }
    pub fn view<'a>(&'a self) -> SubImage<&'a ImageBuffer<Rgb<u8>, Vec<u8>>> {
        match self.crop {
            Some(crop) => {
                self.im_orig
                    .view(crop.x as u32, crop.y as u32, crop.w as u32, crop.h as u32)
            }
            None => self.im_transformed.view(
                0,
                0,
                self.im_transformed.width(),
                self.im_transformed.height(),
            ),
        }
    }

    fn transform_to_match_surface(&mut self, surf_w: u32, surf_h: u32) -> (u32, u32) {
        let w_orig = self.im_orig.width();
        let h_orig = self.im_orig.height();
        let buffer_size = if w_orig > surf_w || h_orig > surf_h {
            let w_ratio = w_orig as f64 / surf_w as f64;
            let h_ratio = h_orig as f64 / surf_h as f64;
            let ratio = w_ratio.max(h_ratio);
            let w_new = (w_orig as f64 / ratio) as u32;
            let h_new = (h_orig as f64 / ratio) as u32;
            self.im_transformed =
                imageops::resize(&self.im_orig, w_new, h_new, FilterType::Nearest);
            (w_new, h_new)
        } else {
            (w_orig, h_orig)
        };
        match self.crop {
            Some(c) => (c.h as u32, c.w as u32),
            None => buffer_size
        }
    }

    fn set_crop(
        &mut self,
        m_press_x: usize,
        m_press_y: usize,
        m_release_x: usize,
        m_release_y: usize,
    ) -> Option<(usize, usize)> {
        let x_min = m_press_x.min(m_release_x);
        let y_min = m_press_y.min(m_release_y);
        let x_max = m_press_x.max(m_release_x);
        let y_max = m_press_y.max(m_release_y);
        let w = x_max - x_min;
        let h = y_max - y_min;
        if w > MIN_CROP && h > MIN_CROP {
            let w_transformed = self.im_transformed.width();
            let h_transformed = self.im_transformed.height();
            let w_orig = self.im_orig.width();
            let h_orig = self.im_orig.height();
            let x_min_t = coord_trans_2_orig(x_min, w_transformed, w_orig);
            let y_min_t = coord_trans_2_orig(y_min, h_transformed, h_orig);
            let x_max_t = coord_trans_2_orig(x_max, w_transformed, w_orig);
            let y_max_t = coord_trans_2_orig(y_max, h_transformed, h_orig);
            
            self.crop = Some(Crop {
                x: x_min_t as usize,
                y: y_min_t as usize,
                w: (x_max_t - x_min_t) as usize,
                h: (y_max_t - y_min_t) as usize,
            });
            Some((w, h))
        } else {
            None
        }
    }

    /// Draw the image to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {
        let sub_image = self.view();
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % sub_image.width() as usize) as u32;
            let y = (i / sub_image.width() as usize) as u32;
            let rgb = sub_image.get_pixel(x, y).0;
            let rgba = [rgb[0], rgb[1], rgb[2], 0xff];

            pixel.copy_from_slice(&rgba);
        }
    }

    fn get_pixel_on_orig(&self, mouse_pos: Option<(usize, usize)>) -> Option<(usize, usize, [u8; 3])> {
        let (x_off, y_off, x_maxp1, y_maxp1) = match &self.crop {
            Some(c) => (
                c.x as u32,
                c.y as u32,
                (c.x + c.w) as u32,
                (c.y + c.h) as u32,
            ),
            _ => (
                0,
                0,
                self.im_transformed.width(),
                self.im_transformed.height(),
            ),
        };
        match mouse_pos {
            Some((x, y)) if x < x_maxp1 as usize && y < y_maxp1 as usize => {
                let x_orig = x_off + coord_trans_2_orig(x, x_maxp1, self.im_orig.width());
                let y_orig = y_off + coord_trans_2_orig(y, y_maxp1, self.im_orig.height());
                Some((
                    x_orig as usize,
                    y_orig as usize,
                    self.im_orig.get_pixel(x as u32, y as u32).0,
                ))
            }
            _ => None,
        }
    }
}

fn coord_trans_2_orig(x: usize, n_transformed: u32, n_orig: u32) -> u32 {
    (x as f64 / n_transformed as f64 * n_orig as f64) as u32
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
    let mut world = World::new(ImageBuffer::<Rgb<u8>, _>::new(START_WIDTH, START_HEIGHT));
    let mut mouse_pressed_pos: Option<(usize, usize)> = None;
    let mut file_selected = None;

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
                if mouse_pressed_pos.is_none() {
                    if let Some((x, y)) = mouse_pos {
                        mouse_pressed_pos = Some((x, y));
                    }
                }
            }
            if input.mouse_released(LEFT_BTN) {
                match (mouse_pressed_pos, mouse_pos) {
                    (Some((c_x, c_y)), Some((m_x, m_y))) => {
                        match world.set_crop(c_x, c_y, m_x, m_y) {
                            Some((w, h)) => pixels.resize_buffer(w as u32, h as u32),
                            None => (),
                        }
                        mouse_pressed_pos = None;
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
                    world = World::new(image_tmp.into_rgb8());
                    let size = window.inner_size();
                    let (w, h) = world.transform_to_match_surface(size.width, size.width);
                    pixels.resize_buffer(w, h);
                }
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                let (w, h) = world.transform_to_match_surface(size.width, size.height);
                pixels.resize_buffer(w, h);
                framework.resize(size.width, size.height);
                pixels.resize_surface(size.width, size.height);
            }

            // show position and rgb value
            if framework.gui().file_selected().is_some() {
                framework.gui().set_state(
                    world.get_pixel_on_orig(mouse_pos),
                    (world.im_orig.width(), world.im_orig.height()),
                );
            } else {
                framework.gui().set_state(None, (0, 0));
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
                world.draw(pixels.get_frame());

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
