#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use image::{ImageBuffer, Rgb};
use log::error;
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;
use world::{Crop, World};

mod cache;
mod gui;
mod reader;
mod ssh;
mod threadpool;
mod result;
mod util;
mod cfg;
mod world;
const START_WIDTH: u32 = 640;
const START_HEIGHT: u32 = 480;

const LEFT_BTN: usize = 0;
const RIGHT_BTN: usize = 1;

fn main() -> Result<(), pixels::Error> {
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
                .window_pos_to_pixel(input.mouse().unwrap_or((-1.0, -1.0)))
                .ok();

            if input.key_pressed(VirtualKeyCode::M) {
                framework.gui().open();
            }

            // crop
            if input.mouse_pressed(LEFT_BTN) || input.mouse_pressed(RIGHT_BTN) {
                if let (None, Some((m_x, m_y))) = (mouse_pressed_start_pos, mouse_pos) {
                    mouse_pressed_start_pos = Some((m_x, m_y));
                }
            }
            if input.mouse_released(LEFT_BTN) {
                if let (Some(mps), Some(mr)) = (mouse_pressed_start_pos, mouse_pos) {
                    world.crop(mps, mr, &window.inner_size());
                    if world.get_crop().is_some() {
                        let (w, h) = world.scale_to_match_win_inner(
                            window.inner_size().width,
                            window.inner_size().height,
                        );
                        pixels.resize_buffer(w, h);
                    }
                }
                mouse_pressed_start_pos = None;
                world.hide_draw_crop();
            }
            // crop move
            if input.mouse_held(RIGHT_BTN) {
                if let (Some(mps), Some(mp)) = (mouse_pressed_start_pos, mouse_pos) {
                    let win_inner = window.inner_size();
                    world.move_crop(mps, mp, &win_inner);
                    world.scale_to_match_win_inner(win_inner.width, win_inner.height);
                    mouse_pressed_start_pos = mouse_pos;
                }
            } else if input.mouse_held(LEFT_BTN) {
                if let (Some((mps_x, mps_y)), Some((m_x, m_y))) =
                    (mouse_pressed_start_pos, mouse_pos)
                {
                    let x_min = mps_x.min(m_x);
                    let y_min = mps_y.min(m_y);
                    let x_max = mps_x.max(m_x);
                    let y_max = mps_y.max(m_y);
                    world.show_draw_crop(Crop {
                        x: x_min as u32,
                        y: y_min as u32,
                        w: (x_max - x_min) as u32,
                        h: (y_max - y_min) as u32,
                    });
                }
            }
            if input.mouse_released(RIGHT_BTN) {
                mouse_pressed_start_pos = None;
            }
            // uncrop
            if input.key_pressed(VirtualKeyCode::Back) {
                world.uncrop();
                let size = window.inner_size();
                let (w, h) = world.scale_to_match_win_inner(size.width, size.height);
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
            let gui_file_selected = framework.gui().file_selected_idx();
            if file_selected != gui_file_selected {
                if let Some(seleceted) = &gui_file_selected {
                    file_selected = gui_file_selected;
                    let old_crop = world.get_crop();
                    let (old_w, old_h) = world.shape_orig();
                    world = World::new(framework.gui().read_image(*seleceted));
                    if (old_w, old_h) == world.shape_orig() {
                        world.apply_crop(&old_crop);
                    }
                    let size = window.inner_size();
                    let (w, h) = world.scale_to_match_win_inner(size.width, size.height);
                    pixels.resize_buffer(w, h);
                }
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                let (w, h) = world.scale_to_match_win_inner(size.width, size.height);
                pixels.resize_buffer(w, h);
                framework.resize(size.width, size.height);
                pixels.resize_surface(size.width, size.height);
            }

            // show position and rgb value
            if framework.gui().file_selected_idx().is_some() {
                let data_point = world.get_pixel_on_orig(mouse_pos, &window.inner_size());
                let (w_orig, h_orig) = world.shape_orig();
                let s = match data_point {
                    Some((x, y, rgb)) => {
                        format!(
                            "Rimview - {}x{} - ({}, {}) -> ({}, {}, {})",
                            w_orig, h_orig, x, y, rgb[0], rgb[1], rgb[2]
                        )
                    }
                    None => format!("Rimview - {}x{} - (x, y) -> (r, g, b)", w_orig, h_orig),
                };
                window.set_title(s.as_str())
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
