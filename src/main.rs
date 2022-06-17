#![deny(clippy::all)]
#![forbid(unsafe_code)]

use image::{DynamicImage, GenericImageView};
use image::{ImageBuffer, Rgb};
use lazy_static::lazy_static;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use rvlib::cfg::{get_cfg, Cfg};
use rvlib::history::{History, Record};
use rvlib::menu::{Framework, Info};
use rvlib::result::{to_rv, RvError, RvResult};
use rvlib::tools::{make_tool_vec, Tool, ToolWrapper};
use rvlib::util::{self, apply_to_matched_image, mouse_pos_transform, Shape};
use rvlib::world::World;
use rvlib::{apply_tool_method, cfg, format_rverr, httpserver};
use std::fmt::Debug;
use std::fs;
use std::mem;
use std::path::Path;
use std::sync::mpsc::Receiver;
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

fn http_address() -> &'static str {
    lazy_static! {
        static ref CFG: Cfg = get_cfg().expect("config broken");
        static ref HTTP_ADDRESS: &'static str = CFG.http_address();
    }
    &HTTP_ADDRESS
}
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
    mut history: History,
    shape_win: Shape,
    mouse_pos: Option<(usize, usize)>,
    event: &util::Event,
    pixels: &mut Pixels,
) -> (World, History) {
    let old_shape = Shape::from_im(world.im_view());
    for t in tools {
        (world, history) =
            apply_tool_method!(t, events_tf, world, history, shape_win, mouse_pos, event);
    }
    let new_shape = Shape::from_im(world.im_view());
    if old_shape != new_shape {
        pixels.resize_buffer(new_shape.w, new_shape.h);
    }
    (world, history)
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

fn increase_port(address: &str) -> RvResult<String> {
    let address_wo_port = address.split(':').next();
    let port = address.split(':').last();
    if let Some(port) = port {
        if let Some(address_wo_port) = address_wo_port {
            Ok(format!(
                "{}:{}",
                address_wo_port,
                (port.parse::<usize>().map_err(to_rv)? + 1)
            ))
        } else {
            Err(format_rverr!("is address of {} missing?", address))
        }
    } else {
        Err(format_rverr!("is port of address {} missing?", address))
    }
}

fn restart_http(
    http_addr: &str,
    mut stop_restarting_http: bool,
) -> (String, bool, Option<Receiver<RvResult<String>>>) {
    
    let http_addr = match increase_port(http_addr) {
        Ok(x) => x,
        Err(e) => {
            println!("{:?}", e);
            stop_restarting_http = true;
            "".to_string()
        }
    };
    if !stop_restarting_http {
        println!("restarting http server with increased port");
        if let Ok((_, rx)) = httpserver::launch(http_addr.clone()) {
            (http_addr, stop_restarting_http, Some(rx))
        } else {
            (http_addr, stop_restarting_http, None)
        }
    } else {
        (http_addr, stop_restarting_http, None)
    }
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
    }

    // application state to create pixels buffer, i.e., everything not part of framework.gui()
    let mut world = empty_world();
    let mut history = History::new();
    let mut tools = make_tool_vec();
    let mut file_selected = None;
    let mut is_loading_screen_active = false;
    let mut undo_redo_load = false;
    let mut rx_opt: Option<Receiver<RvResult<String>>> = None;
    let mut http_addr = http_address().to_string();
    let mut stop_restarting_http = false;
    if let Ok((_, rx)) = httpserver::launch(http_addr.clone()) {
        rx_opt = Some(rx);
    }
    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close application
            if input.quit() {
                *control_flow = ControlFlow::Exit;
                if let Err(e) = remove_tmpdir() {
                    framework
                        .menu_mut()
                        .popup(Info::Error(format!("could not delete tmpdir. {:?}", e)));
                    thread::sleep(Duration::from_secs(5));
                }
                return;
            }

            // update world based on tools
            let shape_win = Shape::from_size(&window.inner_size());
            let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
            let event = util::Event::new(&input);
            if framework.menu().are_tools_active() {
                (world, history) = apply_tools(
                    &mut tools,
                    mem::take(&mut world),
                    mem::take(&mut history),
                    shape_win,
                    mouse_pos,
                    &event,
                    &mut pixels,
                );
            }

            if input.key_pressed(VirtualKeyCode::M) {
                framework.menu_mut().toggle();
            }

            if input.key_pressed(VirtualKeyCode::Right)
                || input.key_pressed(VirtualKeyCode::Down)
                || input.key_pressed(VirtualKeyCode::PageDown)
            {
                framework.menu_mut().next();
            }

            if input.key_pressed(VirtualKeyCode::Left)
                || input.key_pressed(VirtualKeyCode::Up)
                || input.key_pressed(VirtualKeyCode::PageUp)
            {
                framework.menu_mut().prev();
            }

            // check for new image requests from http server
            if let Some(rx) = &rx_opt {
                if let Some(last) = rx.try_iter().last() {
                    match last {
                        Ok(file_label) => framework.menu_mut().select_file_label(&file_label),
                        Err(e) => {
                            println!("{:?}", e);
                            (http_addr, stop_restarting_http, rx_opt) = restart_http(&http_addr, stop_restarting_http);
                        }
                    }
                }
            }

            let menu_file_selected = framework.menu().file_label_selected_idx();
            let make_folder_label = || framework.menu().folder_label().map(|s| s.to_string());
            let opt_rec_new = if (input.key_held(VirtualKeyCode::RControl)
                || input.key_held(VirtualKeyCode::LControl))
                && input.key_pressed(VirtualKeyCode::Z)
            {
                // undo
                undo_redo_load = true;
                Some(history.prev_world(Record {
                    im_orig: std::mem::take(world.im_orig_mut()),
                    file_label_idx: file_selected,
                    folder_label: make_folder_label(),
                }))
            } else if (input.key_held(VirtualKeyCode::RControl)
                || input.key_held(VirtualKeyCode::LControl))
                && input.key_pressed(VirtualKeyCode::Y)
            {
                // redo
                undo_redo_load = true;
                Some(history.next_world(Record {
                    im_orig: std::mem::take(world.im_orig_mut()),
                    file_label_idx: file_selected,
                    folder_label: make_folder_label(),
                }))
            } else if file_selected != menu_file_selected || is_loading_screen_active {
                // load new image
                if let Some(selected) = &menu_file_selected {
                    if !is_loading_screen_active && !undo_redo_load {
                        history.push(Record {
                            im_orig: world.im_orig().clone(),
                            file_label_idx: file_selected,
                            folder_label: make_folder_label(),
                        });
                    }
                    let read_image_and_idx = match framework.menu_mut().read_image(*selected) {
                        Some(ri) => {
                            undo_redo_load = false;
                            file_selected = menu_file_selected;
                            is_loading_screen_active = false;
                            (ri, file_selected)
                        }
                        None => {
                            thread::sleep(Duration::from_millis(20));
                            let shape = world.shape_orig();
                            file_selected = menu_file_selected;
                            is_loading_screen_active = true;
                            (loading_image(shape), file_selected)
                        }
                    };
                    Some(read_image_and_idx)
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((im_orig, file_label_idx)) = opt_rec_new {
                let size = window.inner_size();
                let shape_win = Shape::from_size(&size);
                let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
                let event = util::Event::from_image_loaded(&input);
                if file_label_idx.is_some() {
                    framework.menu_mut().select_label_idx(file_label_idx);
                }
                world = World::new(im_orig);
                (world, history) = apply_tools(
                    &mut tools,
                    mem::take(&mut world),
                    mem::take(&mut history),
                    shape_win,
                    mouse_pos,
                    &event,
                    &mut pixels,
                );
                let Shape { w, h } = Shape::from_im(world.im_view());
                pixels.resize_buffer(w, h);
            }
            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                let shape_win = Shape::from_size(&size);
                if shape_win.h > 0 && shape_win.w > 0 {
                    let event = util::Event::from_window_resized(&input);
                    let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
                    (world, history) = apply_tools(
                        &mut tools,
                        mem::take(&mut world),
                        mem::take(&mut history),
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
            }

            // show position and rgb value
            if let Some(idx) = framework.menu_mut().file_label_selected_idx() {
                let shape_win = Shape {
                    w: window.inner_size().width,
                    h: window.inner_size().height,
                };
                let mouse_pos = mouse_pos_transform(&pixels, input.mouse());
                let data_point = get_pixel_on_orig_str(&mut tools, &world, mouse_pos, shape_win);
                let shape = world.shape_orig();
                let file_label = framework.menu().file_label(idx);
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

#[test]
fn test_increase_port() -> RvResult<()> {
    assert_eq!(increase_port("address:1234")?, "address:1235");
    Ok(())
}
