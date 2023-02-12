#![deny(clippy::all)]
#![forbid(unsafe_code)]

use image::{DynamicImage, GenericImageView};
use image::{ImageBuffer, Rgb};
use lazy_static::lazy_static;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use rvlib::cfg::{self, Cfg};
use rvlib::control::{Control, Info};
use rvlib::domain::{self, zoom_box_mouse_wheel, Shape};
use rvlib::history::History;
use rvlib::menu::Framework;
use rvlib::result::RvResult;
use rvlib::tools::{make_tool_vec, Manipulate, ToolState, ToolWrapper, BBOX_NAME, ZOOM_NAME};
use rvlib::world::World;
use rvlib::{apply_tool_method_mut, defer, httpserver, image_util};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::mem;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::{Event, MouseScrollDelta, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const START_WIDTH: u32 = 640;
const START_HEIGHT: u32 = 480;
const MIN_WIN_INNER_SIZE: LogicalSize<i32> = LogicalSize::new(32, 32);

fn cfg_static_ref() -> &'static Cfg {
    lazy_static! {
        static ref CFG: Cfg = cfg::get_cfg().expect("config broken");
    }
    &CFG
}

fn http_address() -> &'static str {
    lazy_static! {
        static ref HTTP_ADDRESS: &'static str = cfg_static_ref().http_address();
    }
    &HTTP_ADDRESS
}
fn pos_2_string_gen<T>(im: &T, x: u32, y: u32) -> String
where
    T: GenericImageView,
    <T as GenericImageView>::Pixel: Debug,
{
    let p = im.get_pixel(x, y);
    format!("({x}, {y}) -> ({p:?})")
}

fn pos_2_string(im: &DynamicImage, x: u32, y: u32) -> String {
    image_util::apply_to_matched_image(
        im,
        |im| pos_2_string_gen(im, x, y),
        |im| pos_2_string_gen(im, x, y),
        |im| pos_2_string_gen(im, x, y),
        |im| pos_2_string_gen(im, x, y),
    )
}

fn get_pixel_on_orig_str(
    world: &World,
    mouse_pos: Option<(usize, usize)>,
    shape_win: Shape,
) -> Option<String> {
    domain::mouse_pos_to_orig_pos(mouse_pos, world.data.shape(), shape_win, world.zoom_box())
        .map(|(x, y)| pos_2_string(world.data.im_background(), x, y))
}

fn apply_tools(
    tools: &mut Vec<ToolState>,
    mut world: World,
    mut history: History,
    shape_win: Shape,
    mouse_pos: Option<(usize, usize)>,
    input_event: &WinitInputHelper,
) -> (World, History) {
    for t in tools {
        if t.is_active() {
            (world, history) = apply_tool_method_mut!(
                t,
                events_tf,
                world,
                history,
                shape_win,
                mouse_pos,
                input_event
            );
        }
    }
    (world, history)
}

fn remove_tmpdir() {
    match cfg::get_cfg() {
        Ok(cfg) => {
            match cfg.tmpdir() {
                Ok(td) => match fs::remove_dir_all(Path::new(td)) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("couldn't remove tmpdir {e:?}")
                    }
                },
                Err(e) => {
                    println!("couldn't remove tmpdir {e:?}")
                }
            };
        }
        Err(e) => {
            println!("could not load cfg {e:?}");
        }
    };
}

macro_rules! activate_tool_event {
    ($key:ident, $name:expr, $input:expr, $rat:expr, $tools:expr) => {
        if $input.held_alt() && $input.key_pressed(VirtualKeyCode::$key) {
            $rat = Some(
                $tools
                    .iter()
                    .enumerate()
                    .find(|(_, t)| t.name == $name)
                    .unwrap()
                    .0,
            );
        }
    };
}

fn main() -> Result<(), pixels::Error> {
    defer!(remove_tmpdir);
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(START_WIDTH as f64, START_HEIGHT as f64);
        WindowBuilder::new()
            .with_title("RV Image")
            .with_inner_size(size)
            .with_min_inner_size(MIN_WIN_INNER_SIZE)
            .build(&event_loop)
            .unwrap()
    };
    let mut tools = make_tool_vec();
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
        World::from_real_im(
            DynamicImage::ImageRgb8(ImageBuffer::<Rgb<u8>, _>::new(START_WIDTH, START_HEIGHT)),
            HashMap::new(),
            "".to_string(),
            Shape::new(START_WIDTH, START_HEIGHT),
        )
    }

    // application state to create pixels buffer, i.e., everything not part of framework.gui()
    let mut world = empty_world();
    let mut ctrl = Control::new(cfg::get_cfg().unwrap_or_else(|e| {
        println!("could not read cfg due to {e:?}, returning default");
        cfg::get_default_cfg()
    }));
    let mut history = History::new();
    let mut recently_activated_tool_idx = None;
    // http server state
    let mut rx_from_http: Option<Receiver<RvResult<String>>> = None;
    let mut http_addr = http_address().to_string();
    if let Ok((_, rx)) = httpserver::launch(http_addr.clone()) {
        rx_from_http = Some(rx);
    }
    event_loop.run(move |event, _, control_flow| {
        match cfg_static_ref().connection {
            cfg::Connection::Ssh => {
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(300));
            }
            _ => {
                *control_flow = ControlFlow::Poll;
            }
        }
        let shape_win = Shape::from_size(&window.inner_size());
        let mouse_pos = domain::mouse_pos_transform(&pixels, input.mouse());
        // Handle input events
        if input.update(&event) {
            // Close application
            if input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // update world based on tools
            if recently_activated_tool_idx.is_none() {
                recently_activated_tool_idx = framework.recently_activated_tool();
            }
            if let (Some(idx_active), Some(_)) =
                (recently_activated_tool_idx, &world.data.meta_data.file_path)
            {
                if !ctrl.flags().is_loading_screen_active {
                    for (i, t) in tools.iter_mut().enumerate() {
                        if i == idx_active {
                            (world, history) = t.activate(
                                mem::take(&mut world),
                                mem::take(&mut history),
                                shape_win,
                            );
                        } else {
                            let meta_data = ctrl.meta_data(
                                ctrl.file_selected_idx,
                                Some(ctrl.flags().is_loading_screen_active),
                            );
                            world.data.meta_data = meta_data;
                            (world, history) = t.deactivate(
                                mem::take(&mut world),
                                mem::take(&mut history),
                                shape_win,
                            );
                        }
                    }
                    recently_activated_tool_idx = None;
                }
            }

            if input.held_alt() && input.key_pressed(VirtualKeyCode::Q) {
                println!("deactivate all tools");
                for t in tools.iter_mut() {
                    let meta_data = ctrl.meta_data(
                        ctrl.file_selected_idx,
                        Some(ctrl.flags().is_loading_screen_active),
                    );
                    world.data.meta_data = meta_data;
                    (world, history) =
                        t.deactivate(mem::take(&mut world), mem::take(&mut history), shape_win);
                }
            }
            activate_tool_event!(B, BBOX_NAME, input, recently_activated_tool_idx, tools);
            activate_tool_event!(Z, ZOOM_NAME, input, recently_activated_tool_idx, tools);

            if input.held_control() && input.key_pressed(VirtualKeyCode::T) {
                framework.toggle_tools_menu();
            }
            if input.held_control() && input.key_pressed(VirtualKeyCode::M) {
                framework.menu_mut().toggle();
            }
            if input.key_released(VirtualKeyCode::F5) {
                if let Err(e) = ctrl.reload() {
                    framework
                        .menu_mut()
                        .show_info(Info::Error(format!("{e:?}")));
                }
            }
            if input.key_pressed(VirtualKeyCode::PageDown) {
                ctrl.paths_navigator.next();
            }
            if input.key_pressed(VirtualKeyCode::PageUp) {
                ctrl.paths_navigator.prev();
            }
            if input.key_pressed(VirtualKeyCode::Escape) {
                world.set_zoom_box(None, shape_win);
            }

            // check for new image requests from http server
            let rx_match = &rx_from_http.as_ref().map(|rx| rx.try_iter().last());
            if let Some(Some(Ok(file_label))) = rx_match {
                ctrl.paths_navigator.select_file_label(file_label);
                ctrl.paths_navigator.activate_scroll_to_selected_label();
            } else if let Some(Some(Err(e))) = rx_match {
                // if the server thread sends an error we restart the server
                println!("{e:?}");
                (http_addr, rx_from_http) =
                    match httpserver::restart_with_increased_port(&http_addr) {
                        Ok(x) => x,
                        Err(e) => {
                            println!("{e:?}");
                            (http_addr.to_string(), None)
                        }
                    };
            }

            // load new image if requested by a menu click or by the http server
            let ims_raw_idx_pair = if input.held_control() && input.key_pressed(VirtualKeyCode::Z) {
                ctrl.undo(&mut history)
            } else if input.held_control() && input.key_pressed(VirtualKeyCode::Y) {
                ctrl.redo(&mut history)
            } else {
                match ctrl.load_new_image_if_triggered(&world, &mut history) {
                    Ok(iip) => iip,
                    Err(e) => {
                        framework
                            .menu_mut()
                            .show_info(Info::Error(format!("{e:?}")));
                        None
                    }
                }
            };

            if let Some((ims_raw, file_label_idx)) = ims_raw_idx_pair {
                let size = window.inner_size();
                let shape_win = Shape::from_size(&size);
                if file_label_idx.is_some() {
                    ctrl.paths_navigator.select_label_idx(file_label_idx);
                }
                let zoom_box = if ims_raw.shape() == world.data.shape() {
                    *world.zoom_box()
                } else {
                    None
                };
                world = World::new(ims_raw, zoom_box, shape_win);

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
                    world.update_view(shape_win);
                    let Shape { w, h } = Shape::from_im(world.im_view());
                    pixels.resize_buffer(w, h);
                    framework.resize(size.width, size.height);
                    pixels.resize_surface(size.width, size.height);
                }
            }

            if framework.are_tools_active() {
                let meta_data = ctrl.meta_data(
                    ctrl.file_selected_idx,
                    Some(ctrl.flags().is_loading_screen_active),
                );
                world.data.meta_data = meta_data;
                (world, history) = apply_tools(
                    &mut tools,
                    mem::take(&mut world),
                    mem::take(&mut history),
                    shape_win,
                    mouse_pos,
                    &input,
                );
            }

            // show position and rgb value
            if let Some(idx) = ctrl.paths_navigator.file_label_selected_idx() {
                let shape_win = Shape {
                    w: window.inner_size().width,
                    h: window.inner_size().height,
                };
                let mouse_pos = domain::mouse_pos_transform(&pixels, input.mouse());
                let data_point = get_pixel_on_orig_str(&world, mouse_pos, shape_win);
                let shape = world.shape_orig();
                let file_label = ctrl.file_label(idx);
                let active_tool = tools.iter().find(|t| t.is_active());
                let tool_string = if let Some(t) = active_tool {
                    format!(" - {} tool is active - ", t.name)
                } else {
                    "".to_string()
                };
                let s = match data_point {
                    Some(s) => {
                        format!(
                            "RV Image{} - {} - {}x{} - {}",
                            tool_string, file_label, shape.w, shape.h, s
                        )
                    }
                    None => format!(
                        "RV Image{} - {} - {}x{} - (x, y) -> (r, g, b)",
                        tool_string, file_label, shape.w, shape.h
                    ),
                };
                window.set_title(s.as_str())
            }
            window.request_redraw();
        }

        match event {
            Event::WindowEvent { event, .. } => {
                if input.held_control() {
                    if let WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y_delta),
                        ..
                    } = event
                    {
                        let zb =
                            zoom_box_mouse_wheel(*world.zoom_box(), world.data.shape(), y_delta);
                        world.set_zoom_box(zb, shape_win);
                    }
                }
                // Update egui inputs
                framework.handle_event(&event);
            }
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // Draw the world
                world.draw(&mut pixels);

                // Prepare egui
                framework.prepare(
                    &window,
                    &mut tools,
                    &mut world.data.tools_data_map,
                    &mut ctrl,
                );

                // Render everything together
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // Render the world texture
                    context.scaling_renderer.render(encoder, render_target);

                    // Render egui
                    framework.render(encoder, render_target, context);

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
