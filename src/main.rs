#![deny(clippy::all)]
#![forbid(unsafe_code)]

use image::{DynamicImage, GenericImageView};
use image::{ImageBuffer, Rgb};
use log::error;
use pixels::{Pixels, SurfaceTexture};
use rvlib::gui::Framework;
use rvlib::gui::Info;
use rvlib::result::RvResult;
use rvlib::tools::{make_tool_vec, Tool, ToolWrapper};
use rvlib::util::{self, apply_to_matched_image, mouse_pos_transform, Shape};
use rvlib::world::World;
use rvlib::{apply_tool_method, cfg};
use std::fmt::Debug;
use std::fs;
use std::mem;
use std::path::Path;
use std::thread;
use std::time::Duration;
use winit::dpi::LogicalSize;
use winit::event::Event;
use winit::event::VirtualKeyCode;
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const START_WIDTH: u32 = 640;
const START_HEIGHT: u32 = 480;

fn pos_2_string_gen<T>(im: &T, x: u32, y: u32) -> String
where
    T: GenericImageView,
    <T as GenericImageView>::Pixel: Debug,
{
    let p = im.get_pixel(x, y);
    format!("({}, {}) -> ({:?})", x, y, p)
}

fn pos_2_string(im: &DynamicImage, x: u32, y: u32) -> String {
    apply_to_matched_image(
        im,
        |im| pos_2_string_gen(im, x, y),
        |im| pos_2_string_gen(im, x, y),
        |im| pos_2_string_gen(im, x, y),
        |im| pos_2_string_gen(im, x, y),
    )
}

fn get_pixel_on_orig_str(
    tools: &mut [ToolWrapper],
    world: &World,
    mouse_pos: Option<(usize, usize)>,
    shape_win: Shape,
) -> Option<String> {
    let mut pos_rgb = mouse_pos.map(|mp| (mp.0 as u32, mp.1 as u32));
    let mut res = None;
    for t in tools {
        let current_pos_rgb = apply_tool_method!(t, coord_tf, world, shape_win, pos_rgb);
        if let Some((x, y)) = current_pos_rgb {
            pos_rgb = Some((x, y));
        }
    }
    if let Some((x, y)) = pos_rgb {
        res = Some(pos_2_string(world.im_orig(), x, y));
    }
    res
}

fn apply_tools<'a>(
    tools: &'a mut Vec<ToolWrapper>,
    mut world: World,
    shape_win: Shape,
    mouse_pos: Option<(usize, usize)>,
    event: &util::Event,
    pixels: &mut Pixels,
) -> World {
    let old_shape = Shape::from_im(world.im_view());
    for t in tools {
        world = apply_tool_method!(t, events_tf, world, shape_win, mouse_pos, event);
    }
    let new_shape = Shape::from_im(world.im_view());
    if old_shape != new_shape {
        pixels.resize_buffer(new_shape.w, new_shape.h);
    }
    world
}

fn loading_image(shape: Shape) -> DynamicImage {
    let centers = [
        (shape.w / 2 - 75, shape.h / 2),
        (shape.w / 2 + 75, shape.h / 2),
        (shape.w / 2, shape.h / 2),
    ];
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(shape.w, shape.h, |x, y| {
        for mid in centers.iter() {
            if (mid.0 as i32 - x as i32).pow(2) + (mid.1 as i32 - y as i32).pow(2) < 100 {
                return image::Rgb([255u8, 255u8, 255u8]);
            }
        }
        if (x / 10) % 2 == 0 {
            image::Rgb([180u8, 180u8, 190u8])
        } else {
            image::Rgb([170u8, 170u8, 180u8])
        }
    }))
}

fn remove_tmpdir() -> RvResult<()> {
    let cfg = cfg::get_cfg()?;
    match cfg.tmpdir() {
        Ok(td) => match fs::remove_dir_all(Path::new(td)) {
            Ok(_) => {}
            Err(e) => {
                println!("couldn't remove tmpdir {:?} ", e)
            }
        },
        Err(e) => {
            println!("couldn't remove tmpdir {:?} ", e)
        }
    };
    Ok(())
}

fn main() -> Result<(), pixels::Error> {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(START_WIDTH as f64, START_HEIGHT as f64);
        WindowBuilder::new()
            .with_title("RV Image")
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

    fn empty_world() -> World {
        World::new(DynamicImage::ImageRgb8(ImageBuffer::<Rgb<u8>, _>::new(
            START_WIDTH,
            START_HEIGHT,
        )))
        .expect("bug, empty world creation needs to work")
    }

    // application state to create pixels buffer, i.e., everything not part of framework.gui()
    let mut world = empty_world();
    let mut tools = make_tool_vec();
    let mut file_selected = None;
    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close application
            if input.quit() {
                *control_flow = ControlFlow::Exit;
                match remove_tmpdir() {
                    Err(e) => {
                        framework
                            .gui()
                            .popup(Info::Error(format!("could not delete tmpdir. {:?}", e)));
                        thread::sleep(Duration::from_secs(5));
                    }
                    _ => (),
                }
                return;
            }

            // update world based on tools
            let shape_win = Shape::from_size(&window.inner_size());
            let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
            let event = util::Event::new(&input);
            if framework.gui().are_tools_active() {
                world = apply_tools(
                    &mut tools,
                    mem::take(&mut world),
                    shape_win,
                    mouse_pos,
                    &event,
                    &mut pixels,
                );
            }

            if input.key_pressed(VirtualKeyCode::M) {
                framework.gui().toggle();
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
                if let Some(selected) = &gui_file_selected {
                    let im_read = match framework.gui().read_image(*selected) {
                        Some(ri) => {
                            file_selected = gui_file_selected;
                            ri
                        }
                        None => {
                            thread::sleep(Duration::from_millis(20));
                            let shape = world.shape_orig();
                            loading_image(shape)
                        }
                    };
                    world = match World::new(im_read) {
                        Ok(w) => w,
                        Err(e) => {
                            framework.gui().popup(Info::Error(e.to_string()));
                            empty_world()
                        }
                    };
                    let size = window.inner_size();
                    let shape_win = Shape::from_size(&size);
                    let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
                    let event = util::Event::from_image_loaded(&input);
                    world = apply_tools(
                        &mut tools,
                        mem::take(&mut world),
                        shape_win,
                        mouse_pos,
                        &event,
                        &mut pixels,
                    );
                    let Shape { w, h } = Shape::from_im(world.im_view());
                    pixels.resize_buffer(w, h);
                }
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                let shape_win = Shape::from_size(&size);
                let event = util::Event::from_window_resized(&input);
                let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
                world = apply_tools(
                    &mut tools,
                    mem::take(&mut world),
                    shape_win,
                    mouse_pos,
                    &event,
                    &mut pixels,
                );
                let Shape { w, h } = Shape::from_im(world.im_view());
                pixels.resize_buffer(w, h);
                framework.resize(size.width, size.height);
                pixels.resize_surface(size.width, size.height);
            }

            // show position and rgb value
            if let Some(idx) = framework.gui().file_selected_idx() {
                let shape_win = Shape {
                    w: window.inner_size().width,
                    h: window.inner_size().height,
                };
                let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
                let data_point = get_pixel_on_orig_str(&mut tools, &world, mouse_pos, shape_win);
                let shape = world.shape_orig();
                let file_label = framework.gui().file_label(idx);
                let s = match data_point {
                    Some(s) => {
                        format!(
                            "RV Image - {} - {}x{} - {}",
                            file_label, shape.w, shape.h, s
                        )
                    }
                    None => format!(
                        "RV Image - {} - {}x{} - (x, y) -> (r, g, b)",
                        file_label, shape.w, shape.h
                    ),
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
