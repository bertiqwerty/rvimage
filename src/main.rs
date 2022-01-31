#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use image::{ImageBuffer, Rgb};
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod gui;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;

/// Representation of the application state. In this example, a box will bounce around the screen.
struct World {
    image: ImageBuffer<Rgb<u8>, Vec<u8>>,
    mouse_x: usize,
    mouse_y: usize,
    rgb: [u8; 3],
}

fn main() -> Result<(), Error> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("Hello Pixels + egui")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)?;
        let framework =
            Framework::new(window_size.width, window_size.height, scale_factor, &pixels);

        (pixels, framework)
    };
    let image = ImageBuffer::new(WIDTH, HEIGHT);
    let mut world = World::new(image);

    let mut file_selected = None;

    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            let framework_file_selected = framework.file_selected();
            if &file_selected != framework_file_selected {
                if let Some(path) = &framework_file_selected {
                    file_selected = framework_file_selected.clone();
                    let image_tmp = image::io::Reader::open(path).unwrap().decode().unwrap();
                    let image = image_tmp.into_rgb8();
                    pixels.resize_buffer(image.width(), image.height());
                    world = World::new(image);
                }
            }

            let mouse_pos = pixels.window_pos_to_pixel(match input.mouse() {
                Some(pos) => pos,
                None => (0.0, 0.0),
            });
            match mouse_pos {
                Ok(p) => world.set_mouse_pos(p.0, p.1),
                _ => (),
            };

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
                framework.resize(size.width, size.height);
            }
            if framework.file_selected().is_some() {
                framework.set_gui_state(world.mouse_pos(), world.rgb());
            } else {
                framework.set_gui_state((0, 0), [0, 0, 0]);
            }
            // Update internal state and request a redraw
            world.update();
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

impl World {
    /// Create a new `World` instance that can draw a moving box.
    fn new(image: ImageBuffer<Rgb<u8>, Vec<u8>>) -> Self {
        Self {
            image,
            mouse_x: 0,
            mouse_y: 0,
            rgb: [0, 0, 0],
        }
    }

    fn set_mouse_pos(&mut self, x: usize, y: usize) {
        self.mouse_x = x;
        self.mouse_y = y;
    }
    pub fn mouse_pos(&self) -> (usize, usize) {
        (self.mouse_x, self.mouse_y)
    }
    pub fn rgb(&self) -> [u8; 3] {
        self.rgb
    }
    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self) {
        let x = self.mouse_x as u32;
        let y = self.mouse_y as u32;
        if x < WIDTH && y < HEIGHT {
            self.rgb = self.image.get_pixel(x, y).0;
        } else {
            self.rgb = [0, 0, 0];
        }
    }
    /// Draw the `World` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % self.image.width() as usize) as i16;
            let y = (i / self.image.width() as usize) as i16;
            let rgb = self.image.get_pixel(x as u32, y as u32).0;
            let rgba = [rgb[0], rgb[1], rgb[2], 0xff];
            
            pixel.copy_from_slice(&rgba);
        }
    }
}
