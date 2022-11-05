#![deny(clippy::all)]
#![forbid(unsafe_code)]

use image::{DynamicImage, GenericImageView};
use image::{ImageBuffer, Rgb};
use lazy_static::lazy_static;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use rvlib::cfg::{self, Cfg};
use rvlib::history::{History, Record};
use rvlib::menu::{Framework, Info};
use rvlib::result::RvResult;
use rvlib::tools::{make_tool_vec, Manipulate, MetaData, ToolState, ToolWrapper};
use rvlib::util::{self, Shape};
use rvlib::world::{DataRaw, World};
use rvlib::{apply_tool_method_mut, httpserver};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::mem;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const START_WIDTH: u32 = 640;
const START_HEIGHT: u32 = 480;
const MIN_WIN_INNER_SIZE: LogicalSize<i32> = LogicalSize::new(32, 32);
const LOAD_ACTOR_NAME: &str = "Load";

fn cfg() -> &'static Cfg {
    lazy_static! {
        static ref CFG: Cfg = cfg::get_cfg().expect("config broken");
    }
    &CFG
}

fn http_address() -> &'static str {
    lazy_static! {
        static ref HTTP_ADDRESS: &'static str = cfg().http_address();
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
    util::apply_to_matched_image(
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
    util::mouse_pos_to_orig_pos(mouse_pos, world.data.shape(), shape_win, world.zoom_box())
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

fn loading_image(shape: Shape, counter: u128) -> DynamicImage {
    let radius = 7i32;
    let centers = [
        (shape.w - 70, shape.h - 20),
        (shape.w - 50, shape.h - 20),
        (shape.w - 30, shape.h - 20),
    ];
    let off_center_dim = |c_idx: usize, counter_mod: usize, rgb: &[u8; 3]| {
        let mut res = *rgb;
        for (rgb_idx, val) in rgb.iter().enumerate() {
            if counter_mod != c_idx {
                res[rgb_idx] = (*val as f32 * 0.7) as u8;
            } else {
                res[rgb_idx] = *val;
            }
        }
        res
    };
    DynamicImage::ImageRgb8(ImageBuffer::from_fn(shape.w, shape.h, |x, y| {
        for (c_idx, ctr) in centers.iter().enumerate() {
            if (ctr.0 as i32 - x as i32).pow(2) + (ctr.1 as i32 - y as i32).pow(2) < radius.pow(2) {
                let counter_mod = ((counter / 5) % 3) as usize;
                return image::Rgb(off_center_dim(c_idx, counter_mod, &[195u8, 255u8, 205u8]));
            }
        }
        image::Rgb([77u8, 77u8, 87u8])
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
    let mut history = History::new();
    let mut file_selected = None;
    let mut is_loading_screen_active = false;
    let mut undo_redo_load = false;
    let mut counter = 0;
    // http server state
    let mut rx_from_http: Option<Receiver<RvResult<String>>> = None;
    let mut http_addr = http_address().to_string();
    if let Ok((_, rx)) = httpserver::launch(http_addr.clone()) {
        rx_from_http = Some(rx);
    }
    event_loop.run(move |event, _, control_flow| {
        match cfg().connection {
            cfg::Connection::Ssh => {
                *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(300));
            }
            _ => {
                *control_flow = ControlFlow::Poll;
            }
        }
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
            let mouse_pos = util::mouse_pos_transform(&pixels, input.mouse());

            if framework.are_tools_active() {
                let meta_data = framework.meta_data(file_selected);
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
            if let (Some(idx_active), Some(_)) = (
                framework.recently_activated_tool(),
                &world.data.meta_data.file_path,
            ) {
                for (i, t) in tools.iter_mut().enumerate() {
                    if i == idx_active {
                        (world, history) =
                            t.activate(mem::take(&mut world), mem::take(&mut history), shape_win);
                    } else {
                        let meta_data = framework.meta_data(file_selected);
                        world.data.meta_data = meta_data;
                        (world, history) =
                            t.deactivate(mem::take(&mut world), mem::take(&mut history), shape_win);
                    }
                }
            }
            if (input.key_held(VirtualKeyCode::RShift) || input.key_held(VirtualKeyCode::LShift))
                && input.key_pressed(VirtualKeyCode::Q)
            {
                for t in tools.iter_mut() {
                    println!("deactivated all tools");
                    let meta_data = framework.meta_data(file_selected);
                    world.data.meta_data = meta_data;
                    (world, history) =
                        t.deactivate(mem::take(&mut world), mem::take(&mut history), shape_win);
                }
            }

            if (input.key_held(VirtualKeyCode::RControl)
                || input.key_held(VirtualKeyCode::LControl))
                && input.key_pressed(VirtualKeyCode::T)
            {
                framework.toggle_tools_menu();
            }
            if (input.key_held(VirtualKeyCode::RControl)
                || input.key_held(VirtualKeyCode::LControl))
                && input.key_pressed(VirtualKeyCode::M)
            {
                framework.menu_mut().toggle();
            }
            if input.key_pressed(VirtualKeyCode::PageDown) {
                framework.menu_mut().next();
            }
            if input.key_pressed(VirtualKeyCode::PageUp) {
                framework.menu_mut().prev();
            }

            // check for new image requests from http server
            let rx_match = &rx_from_http.as_ref().map(|rx| rx.try_iter().last());
            if let Some(Some(Ok(file_label))) = rx_match {
                framework.menu_mut().select_file_label(file_label);
                framework.menu_mut().activate_scroll_to_label();
            } else if let Some(Some(Err(e))) = rx_match {
                // if the server thread sends an error we restart the server
                println!("{:?}", e);
                (http_addr, rx_from_http) =
                    match httpserver::restart_with_increased_port(&http_addr) {
                        Ok(x) => x,
                        Err(e) => {
                            println!("{:?}", e);
                            (http_addr.to_string(), None)
                        }
                    };
            }

            let menu_file_selected = framework.menu().file_label_selected_idx();
            let make_folder_label = || framework.menu().folder_label().map(|s| s.to_string());

            let ims_raw_idx_pair = if (input.key_held(VirtualKeyCode::RControl)
                || input.key_held(VirtualKeyCode::LControl))
                && input.key_pressed(VirtualKeyCode::Z)
            {
                // undo
                undo_redo_load = true;
                history.prev_world(&make_folder_label())
            } else if (input.key_held(VirtualKeyCode::RControl)
                || input.key_held(VirtualKeyCode::LControl))
                && input.key_pressed(VirtualKeyCode::Y)
            {
                // redo
                undo_redo_load = true;
                history.next_world(&make_folder_label())
            } else if file_selected != menu_file_selected || is_loading_screen_active {
                // load new image
                if let Some(selected) = &menu_file_selected {
                    let folder_label = make_folder_label();
                    let file_path = menu_file_selected
                        .and_then(|fs| Some(framework.menu().file_path(fs)?.to_string()));
                    let reload = false;
                    let read_image_and_idx = match (
                        file_path,
                        framework.menu_mut().read_image(*selected, reload),
                    ) {
                        (Some(fp), Some(ri)) => {
                            let ims_raw = DataRaw::new(
                                ri,
                                MetaData::from_filepath(fp),
                                world.data.tools_data_map.clone(),
                            );
                            if !undo_redo_load {
                                history.push(Record {
                                    ims_raw: ims_raw.clone(),
                                    actor: LOAD_ACTOR_NAME,
                                    file_label_idx: file_selected,
                                    folder_label,
                                });
                            }
                            undo_redo_load = false;
                            file_selected = menu_file_selected;
                            is_loading_screen_active = false;
                            (ims_raw, file_selected)
                        }
                        _ => {
                            thread::sleep(Duration::from_millis(20));
                            let shape = world.shape_orig();
                            file_selected = menu_file_selected;
                            is_loading_screen_active = true;
                            (
                                DataRaw::new(
                                    loading_image(shape, counter),
                                    MetaData::default(),
                                    world.data.tools_data_map.clone(),
                                ),
                                file_selected,
                            )
                        }
                    };
                    Some(read_image_and_idx)
                } else {
                    None
                }
            } else {
                None
            };
            counter += 1;
            if counter == u128::MAX {
                counter = 0;
            }
            if let Some((ims_raw, file_label_idx)) = ims_raw_idx_pair {
                let size = window.inner_size();
                let shape_win = Shape::from_size(&size);
                if file_label_idx.is_some() {
                    framework.menu_mut().select_label_idx(file_label_idx);
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

            // show position and rgb value
            if let Some(idx) = framework.menu_mut().file_label_selected_idx() {
                let shape_win = Shape {
                    w: window.inner_size().width,
                    h: window.inner_size().height,
                };
                let mouse_pos = util::mouse_pos_transform(&pixels, input.mouse());
                let data_point = get_pixel_on_orig_str(&world, mouse_pos, shape_win);
                let shape = world.shape_orig();
                let file_label = framework.menu().file_label(idx);
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
                // Update egui inputs
                framework.handle_event(&event);
            }
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // Draw the world
                world.draw(&mut pixels);

                // Prepare egui
                framework.prepare(&window, &mut tools, &mut world.data.tools_data_map);

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
