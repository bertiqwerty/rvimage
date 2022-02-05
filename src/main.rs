#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use image::imageops::FilterType;
use image::{imageops, GenericImage, ImageBuffer, Rgb};
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod gui;

const START_WIDTH: u32 = 640;
const START_HEIGHT: u32 = 480;

const LEFT_BTN: usize = 0;
const RIGHT_BTN: usize = 1;

const MIN_CROP: u32 = 10;

#[derive(Clone, Copy, Debug)]
struct Crop {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// Everything we need to draw
struct World {
    im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>,
    im_view: ImageBuffer<Rgb<u8>, Vec<u8>>,
    crop: Option<Crop>,
    draw_crop: Option<Crop>, // for drawing a crop animation
}

impl World {
    pub fn new(im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>) -> Self {
        Self {
            im_orig: im_orig.clone(),
            im_view: im_orig,
            crop: None,
            draw_crop: None,
        }
    }

    fn shape_unscaled(&self) -> (u32, u32) {
        match self.crop {
            Some(c) => (c.w as u32, c.h as u32),
            None => (self.im_orig.width(), self.im_orig.height()),
        }
    }

    /// shape of the image that fits into the window
    fn shape_scaled(
        &self,
        w_unscaled: u32,
        h_unscaled: u32,
        w_win_inner: u32,
        h_win_inner: u32,
    ) -> (u32, u32) {
        let w_ratio = w_unscaled as f64 / w_win_inner as f64;
        let h_ratio = h_unscaled as f64 / h_win_inner as f64;
        let ratio = w_ratio.max(h_ratio);
        let w_new = (w_unscaled as f64 / ratio) as u32;
        let h_new = (h_unscaled as f64 / ratio) as u32;
        (w_new, h_new)
    }

    fn scale_to_match_surface(&mut self, w_win_inner: u32, h_win_inner: u32) -> (u32, u32) {
        let (w_unscaled, h_unscaled) = self.shape_unscaled();
        let (w_new, h_new) = self.shape_scaled(w_unscaled, h_unscaled, w_win_inner, h_win_inner);

        match self.crop {
            Some(c) => {
                let cropped_view = self.im_orig.sub_image(c.x, c.y, c.w, c.h);
                let im_cropped = cropped_view.to_image();
                self.im_view = imageops::resize(&im_cropped, w_new, h_new, FilterType::Nearest);
            }
            None => {
                if w_unscaled > w_win_inner || h_unscaled > h_win_inner {
                    self.im_view =
                        imageops::resize(&self.im_orig, w_new, h_new, FilterType::Nearest);
                }
            }
        }

        (w_new, h_new)
    }

    fn make_crop(
        &self,
        mouse_start_x: usize,
        mouse_start_y: usize,
        mouse_end_x: usize,
        mouse_end_y: usize,
        win_inner: &PhysicalSize<u32>,
    ) -> Option<Crop> {
        let prs_orig = self.get_pos_on_orig(Some((mouse_start_x, mouse_start_y)), &win_inner);
        let rel_orig = self.get_pos_on_orig(Some((mouse_end_x, mouse_end_y)), &win_inner);

        match (prs_orig, rel_orig) {
            (Some((px, py)), Some((rx, ry))) => {
                let x_min = px.min(rx) as u32;
                let y_min = py.min(ry) as u32;
                let x_max = px.max(rx) as u32;
                let y_max = py.max(ry) as u32;

                let w = x_max - x_min;
                let h = y_max - y_min;
                if w >= MIN_CROP && h >= MIN_CROP {
                    Some(Crop {
                        x: x_min,
                        y: y_min,
                        w,
                        h,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn move_crop(
        &mut self,
        m_press_x: usize,
        m_press_y: usize,
        m_held_x: usize,
        m_held_y: usize,
        win_inner: &PhysicalSize<u32>,
    ) {
        if let Some(c) = self.crop {
            let press_orig = self.get_pos_on_orig(Some((m_press_x, m_press_y)), &win_inner);
            let held_orig = self.get_pos_on_orig(Some((m_held_x, m_held_y)), &win_inner);
            match (press_orig, held_orig) {
                (Some((px, py)), Some((hx, hy))) => {
                    let x_shift: i32 = px as i32 - hx as i32;
                    let y_shift: i32 = py as i32 - hy as i32;
                    let x_new = c.x as i32 + x_shift;
                    let y_new = c.y as i32 + y_shift;
                    if x_new >= 0
                        && y_new >= 0
                        && x_new as u32 + c.w < self.im_orig.width()
                        && y_new as u32 + c.h < self.im_orig.height()
                    {
                        self.crop = Some(Crop {
                            x: x_new as u32,
                            y: y_new as u32,
                            w: c.w,
                            h: c.h,
                        });
                    }
                }
                _ => (),
            }
        }
    }

    /// Draw the image to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, pixels: &mut Pixels) {
        let frame_len = pixels.get_frame().len() as u32;
        if frame_len != self.im_view.width() * self.im_view.height() * 4 {
            pixels.resize_buffer(self.im_view.width(), self.im_view.height())
        }
        let frame = pixels.get_frame();

        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % self.im_view.width() as usize) as u32;
            let y = (i / self.im_view.width() as usize) as u32;
            let rgb = self.im_view.get_pixel(x, y).0;
            let rgb_changed = if let Some(dc) = self.draw_crop {
                let offset = 50;
                let change = |x| if 255 - x > offset { x + offset } else { 255 };
                if x >= dc.x && y >= dc.y && x < dc.x + dc.w && y < dc.y + dc.h {
                    [change(rgb[0]), change(rgb[1]), change(rgb[2])]
                } else {
                    rgb
                }
            } else {
                rgb
            };
            let rgba = [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff];

            pixel.copy_from_slice(&rgba);
        }
    }

    fn get_pos_on_orig(
        &self,
        mouse_pos: Option<(usize, usize)>,
        size_win_inner: &PhysicalSize<u32>,
    ) -> Option<(usize, usize)> {
        let (w_unscaled, h_unscaled) = self.shape_unscaled();
        let (w_win_inner, h_win_inner) = (size_win_inner.width, size_win_inner.height);

        let (w_scaled, h_scaled) =
            self.shape_scaled(w_unscaled, h_unscaled, w_win_inner, h_win_inner);

        let (x_off, y_off) = match &self.crop {
            Some(c) => (c.x, c.y),
            _ => (0, 0),
        };
        match mouse_pos {
            Some((x, y)) => {
                let x_orig = x_off + coord_trans_2_orig(x as u32, w_scaled, w_unscaled);
                let y_orig = y_off + coord_trans_2_orig(y as u32, h_scaled, h_unscaled);
                if x_orig < self.im_orig.width() && y_orig < self.im_orig.height() {
                    Some((x_orig as usize, y_orig as usize))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn get_pixel_on_orig(
        &self,
        mouse_pos: Option<(usize, usize)>,
        size_win_inner: &PhysicalSize<u32>,
    ) -> Option<(usize, usize, [u8; 3])> {
        let pos = self.get_pos_on_orig(mouse_pos, size_win_inner);
        pos.map(|(x, y)| (x, y, self.im_orig.get_pixel(x as u32, y as u32).0))
    }
}

fn coord_trans_2_orig(x: u32, n_transformed: u32, n_orig: u32) -> u32 {
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
    let mut mouse_pressed_start_pos: Option<(usize, usize)> = None;
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
            if input.mouse_pressed(LEFT_BTN) || input.mouse_pressed(RIGHT_BTN) {
                match (mouse_pressed_start_pos, mouse_pos) {
                    (None, Some((m_x, m_y))) => {
                        mouse_pressed_start_pos = Some((m_x, m_y));
                    }
                    _ => (),
                }
            }
            if input.mouse_released(LEFT_BTN) {
                match (mouse_pressed_start_pos, mouse_pos) {
                    (Some((mps_x, mps_y)), Some((mrp_x, mrp_y))) => {
                        world.crop =
                            world.make_crop(mps_x, mps_y, mrp_x, mrp_y, &window.inner_size());
                        if world.crop.is_some() {
                            let (w, h) = world.scale_to_match_surface(
                                window.inner_size().width,
                                window.inner_size().height,
                            );
                            pixels.resize_buffer(w, h);
                        }
                        world.draw_crop = None;
                        mouse_pressed_start_pos = None;
                    }
                    _ => (),
                }
            }
            // crop move
            if input.mouse_held(RIGHT_BTN) {
                match (mouse_pressed_start_pos, mouse_pos) {
                    (Some((mpp_x, mpp_y)), Some((mp_x, mp_y))) => {
                        let win_inner = window.inner_size();

                        world.move_crop(mpp_x, mpp_y, mp_x, mp_y, &win_inner);
                        world.scale_to_match_surface(win_inner.width, win_inner.height);
                        mouse_pressed_start_pos = mouse_pos;
                    }
                    _ => (),
                }
            } else if input.mouse_held(LEFT_BTN) {
                match (mouse_pressed_start_pos, mouse_pos) {
                    (Some((mps_x, mps_y)), Some((m_x, m_y))) => {
                        let x_min = mps_x.min(m_x);
                        let y_min = mps_y.min(m_y);
                        let x_max = mps_x.max(m_x);
                        let y_max = mps_y.max(m_y);
                        world.draw_crop = Some(Crop {
                            x: x_min as u32,
                            y: y_min as u32,
                            w: (x_max - x_min) as u32,
                            h: (y_max - y_min) as u32,
                        });
                    }
                    _ => (),
                }
            }
            if input.mouse_released(RIGHT_BTN) {
                mouse_pressed_start_pos = None;
            }
            // uncrop
            if input.key_pressed(VirtualKeyCode::Back) {
                world.crop = None;
                let size = window.inner_size();
                let (w, h) = world.scale_to_match_surface(size.width, size.height);
                pixels.resize_buffer(w, h);
            }

            if input.key_pressed(VirtualKeyCode::Right)
                || input.key_pressed(VirtualKeyCode::Down)
                || input.key_pressed(VirtualKeyCode::PageDown)
            {
                framework.gui().next();
            }

            if input.key_pressed(VirtualKeyCode::Left)
                || input.key_pressed(VirtualKeyCode::Up)
                || input.key_pressed(VirtualKeyCode::PageUp)
            {
                framework.gui().prev();
            }

            // load new image
            let gui_file_selected = framework.gui().file_selected();
            if file_selected != gui_file_selected {
                if let Some(path) = &gui_file_selected {
                    file_selected = gui_file_selected.clone();
                    let image_tmp = image::io::Reader::open(path).unwrap().decode().unwrap();
                    let old_crop = world.crop;
                    let (old_w, old_h) = (world.im_orig.width(), world.im_orig.height());
                    world = World::new(image_tmp.into_rgb8());
                    if (old_w, old_h) == (world.im_orig.width(), world.im_orig.height()) {
                        world.crop = old_crop;
                    }
                    let size = window.inner_size();
                    let (w, h) = world.scale_to_match_surface(size.width, size.height);
                    pixels.resize_buffer(w, h);
                }
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                let (w, h) = world.scale_to_match_surface(size.width, size.height);
                pixels.resize_buffer(w, h);
                framework.resize(size.width, size.height);
                pixels.resize_surface(size.width, size.height);
            }

            // show position and rgb value
            if framework.gui().file_selected().is_some() {
                framework.gui().set_state(
                    world.get_pixel_on_orig(mouse_pos, &window.inner_size()),
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
                world.draw(&mut pixels);

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

#[test]
fn test_world() {
    {
        // some general basic tests
        let (w, h) = (100, 100);
        let win_inner = PhysicalSize::<u32>::new(w, h);
        let mut im = ImageBuffer::<Rgb<u8>, _>::new(w, h);
        im[(10, 10)] = Rgb::<u8>::from([4, 4, 4]);
        im[(20, 30)] = Rgb::<u8>::from([5, 5, 5]);
        let world = World::new(im);
        assert_eq!((w, h), world.shape_unscaled());
        let crop = world.make_crop(10, 10, 60, 60, &win_inner).unwrap();
        assert_eq!(Some((50, 50)), Some((crop.w, crop.h)));
        assert_eq!(
            Some((10, 10, [4, 4, 4])),
            world.get_pixel_on_orig(Some((0, 0)), &win_inner)
        );
        assert_eq!(
            Some((20, 30, [5, 5, 5])),
            world.get_pixel_on_orig(Some((20, 40)), &win_inner)
        );
        assert_eq!((100, 100), (world.im_view.width(), world.im_view.height()));
    }
    {
        // another test on finding pixels in the original image
        let (win_w, win_h) = (200, 100);
        let win_inner = PhysicalSize::<u32>::new(win_w, win_h);
        let (w_im_o, h_im_o) = (100, 50);
        let im = ImageBuffer::<Rgb<u8>, _>::new(w_im_o, h_im_o);
        let world = World::new(im);
        let crop = world.make_crop(10, 20, 50, 40, &win_inner).unwrap();
        assert_eq!(Some((20, 10)), Some((crop.w, crop.h)));
    }
}
