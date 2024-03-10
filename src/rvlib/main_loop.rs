#![deny(clippy::all)]
#![forbid(unsafe_code)]
use crate::cfg::{self, Cfg};
use crate::control::{Control, Info};
use crate::domain::PtI;
use crate::drawme::ImageInfo;
use crate::events::{Events, KeyCode};
use crate::file_util::DEFAULT_PRJ_PATH;
use crate::history::{History, Record};
use crate::menu::{are_tools_active, Menu, ToolSelectMenu};
use crate::result::{trace_ok, RvResult};
use crate::tools::{make_tool_vec, Manipulate, ToolState, ToolWrapper, BBOX_NAME, ZOOM_NAME};
use crate::util::Visibility;
use crate::world::World;
use crate::{apply_tool_method_mut, httpserver, image_util, UpdateView};
use egui::Context;
use image::{DynamicImage, GenericImageView};
use image::{ImageBuffer, Rgb};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;
use std::{fs, mem, thread};
use tracing::{error, info, warn};

const START_WIDTH: u32 = 640;
const START_HEIGHT: u32 = 480;

const AUTOSAVE_INTERVAL_S: u64 = 120;

fn cfg_static_ref() -> &'static Cfg {
    lazy_static! {
        static ref CFG: Cfg = cfg::read_cfg().expect("config broken");
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
    if x < im.width() && y < im.height() {
        image_util::apply_to_matched_image(
            im,
            |im| pos_2_string_gen(im, x, y),
            |im| pos_2_string_gen(im, x, y),
            |im| pos_2_string_gen(im, x, y),
            |im| pos_2_string_gen(im, x, y),
        )
    } else {
        "".to_string()
    }
}

fn get_pixel_on_orig_str(world: &World, mouse_pos: &Option<PtI>) -> Option<String> {
    mouse_pos.map(|p| pos_2_string(world.data.im_background(), p.x, p.y))
}

fn apply_tools(
    tools: &mut Vec<ToolState>,
    mut world: World,
    mut history: History,
    input_event: &Events,
) -> (World, History) {
    for t in tools {
        if t.is_active() {
            (world, history) = apply_tool_method_mut!(t, events_tf, world, history, input_event);
        }
    }
    (world, history)
}

macro_rules! activate_tool_event {
    ($key:ident, $name:expr, $input:expr, $rat:expr, $tools:expr) => {
        if $input.held_alt() && $input.pressed(KeyCode::$key) {
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

fn empty_world() -> World {
    World::from_real_im(
        DynamicImage::ImageRgb8(ImageBuffer::<Rgb<u8>, _>::new(START_WIDTH, START_HEIGHT)),
        HashMap::new(),
        None,
        None,
    )
}

fn find_active_tool(tools: &[ToolState]) -> Option<&str> {
    tools
        .iter()
        .find(|t| t.is_active() && !t.is_always_active())
        .map(|t| t.name)
}

pub struct MainEventLoop {
    menu: Menu,
    tools_select_menu: ToolSelectMenu,
    world: World,
    ctrl: Control,
    history: History,
    tools: Vec<ToolState>,
    recently_clicked_tool_idx: Option<usize>,
    rx_from_http: Option<Receiver<RvResult<String>>>,
    http_addr: String,
    autosave_timer: Instant,
}
impl Default for MainEventLoop {
    fn default() -> Self {
        let cfg = cfg::read_cfg().unwrap_or_else(|e| {
            warn!("could not read cfg due to {e:?}, returning default");
            cfg::get_default_cfg()
        });
        let file_path = std::env::args().nth(1).map(PathBuf::from);
        Self::new(cfg, file_path)
    }
}
impl MainEventLoop {
    pub fn new(cfg: Cfg, prj_file_path: Option<PathBuf>) -> Self {
        let ctrl = Control::new(cfg);

        let mut world = empty_world();
        let mut tools = make_tool_vec();
        for t in &mut tools {
            if t.is_active() {
                (world, _) = t.activate(world, History::default());
            }
        }
        let http_addr = http_address().to_string();
        // http server state
        let rx_from_http = if let Ok((_, rx)) = httpserver::launch(http_addr.clone()) {
            Some(rx)
        } else {
            None
        };
        let mut self_ = Self {
            world,
            ctrl,
            tools,
            http_addr,
            tools_select_menu: ToolSelectMenu::default(),
            menu: Menu::default(),
            history: History::default(),
            recently_clicked_tool_idx: None,
            rx_from_http,
            autosave_timer: Instant::now(),
        };

        trace_ok(self_.load_prj(prj_file_path));
        self_
    }
    pub fn one_iteration(&mut self, e: &Events, ctx: &Context) -> RvResult<UpdateView> {
        let project_loaded = self.menu.ui(
            ctx,
            &mut self.ctrl,
            &mut self.world.data.tools_data_map,
            find_active_tool(&self.tools),
        );
        if project_loaded {
            for t in &mut self.tools {
                self.world = t.deactivate(mem::take(&mut self.world));
            }
        }
        if let Some(elf) = &self.ctrl.log_export_path {
            self.ctrl.export_logs(elf);
        }
        if self.ctrl.log_export_path.is_some() {
            self.ctrl.log_export_path = None;
        }
        if e.held_ctrl() && e.pressed(KeyCode::S) {
            let prj_path = self.ctrl.cfg.current_prj_path().to_path_buf();
            if let Err(e) = self
                .ctrl
                .save(prj_path, &self.world.data.tools_data_map, true)
            {
                self.menu
                    .show_info(Info::Error(format!("could not save project due to {e:?}")));
            }
        }
        egui::SidePanel::right("my_panel")
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    self.tools_select_menu.ui(
                        ui,
                        &mut self.tools,
                        &mut self.world.data.tools_data_map,
                    )
                })
                .inner
            })
            .inner?;

        // tool activation
        if self.recently_clicked_tool_idx.is_none() {
            self.recently_clicked_tool_idx = self.tools_select_menu.recently_clicked_tool();
        }
        if let (Some(idx_active), Some(_)) = (
            self.recently_clicked_tool_idx,
            &self.world.data.meta_data.file_path,
        ) {
            if !self.ctrl.flags().is_loading_screen_active {
                // first deactivate, then activate
                for (i, t) in self.tools.iter_mut().enumerate() {
                    if i != idx_active && t.is_active() && !t.is_always_active() {
                        let meta_data = self.ctrl.meta_data(
                            self.ctrl.file_selected_idx,
                            Some(self.ctrl.flags().is_loading_screen_active),
                        );
                        self.world.data.meta_data = meta_data;
                        self.world = t.deactivate(mem::take(&mut self.world));
                    }
                }
                for (i, t) in self.tools.iter_mut().enumerate() {
                    if i == idx_active {
                        (self.world, self.history) =
                            t.activate(mem::take(&mut self.world), mem::take(&mut self.history));
                    }
                }
                self.recently_clicked_tool_idx = None;
            }
        }

        if e.held_alt() && e.pressed(KeyCode::Q) {
            info!("deactivate all tools");
            let was_any_tool_active = self
                .tools
                .iter()
                .any(|t| t.is_active() && !t.is_always_active());
            for t in self.tools.iter_mut() {
                if !t.is_always_active() && t.is_active() {
                    let meta_data = self.ctrl.meta_data(
                        self.ctrl.file_selected_idx,
                        Some(self.ctrl.flags().is_loading_screen_active),
                    );
                    self.world.data.meta_data = meta_data;
                    self.world = t.deactivate(mem::take(&mut self.world));
                }
            }
            if was_any_tool_active {
                self.history
                    .push(Record::new(self.world.clone(), "deactivation of all tools"));
            }
        }
        // tool activation keyboard shortcuts
        activate_tool_event!(B, BBOX_NAME, e, self.recently_clicked_tool_idx, self.tools);
        activate_tool_event!(Z, ZOOM_NAME, e, self.recently_clicked_tool_idx, self.tools);

        if e.held_ctrl() && e.pressed(KeyCode::M) {
            self.menu.toggle();
        }
        if e.released(KeyCode::F5) {
            if let Err(e) = self.ctrl.reload(self.menu.sort_type()) {
                self.menu
                    .show_info(Info::Error(format!("could not reload due to {e:?}")));
            }
        }
        if e.pressed(KeyCode::PageDown) {
            self.ctrl.paths_navigator.next();
        }
        if e.pressed(KeyCode::PageUp) {
            self.ctrl.paths_navigator.prev();
        }
        if e.pressed(KeyCode::Escape) {
            self.world.set_zoom_box(None);
        }

        // check for new image requests from http server
        let rx_match = &self.rx_from_http.as_ref().map(|rx| rx.try_iter().last());
        if let Some(Some(Ok(file_label))) = rx_match {
            self.ctrl.paths_navigator.select_file_label(file_label);
            self.ctrl
                .paths_navigator
                .activate_scroll_to_selected_label();
        } else if let Some(Some(Err(e))) = rx_match {
            // if the server thread sends an error we restart the server
            warn!("{e:?}");
            (self.http_addr, self.rx_from_http) =
                match httpserver::restart_with_increased_port(&self.http_addr) {
                    Ok(x) => x,
                    Err(e) => {
                        error!("{e:?}");
                        (self.http_addr.to_string(), None)
                    }
                };
        }

        // load new image if requested by a menu click or by the http server
        let world_idx_pair = if e.held_ctrl() && e.pressed(KeyCode::Z) {
            info!("undo");
            self.ctrl.undo(&mut self.history)
        } else if e.held_ctrl() && e.pressed(KeyCode::Y) {
            info!("redo");
            self.ctrl.redo(&mut self.history)
        } else {
            match self
                .ctrl
                .load_new_image_if_triggered(&self.world, &mut self.history)
            {
                Ok(iip) => iip,
                Err(e) => {
                    self.menu.show_info(Info::Error(format!("{e:?}")));
                    None
                }
            }
        };

        if let Some((world, file_label_idx)) = world_idx_pair {
            self.world = world;
            if let Some(active_tool_name) = find_active_tool(&self.tools) {
                self.world
                    .request_redraw_annotations(active_tool_name, Visibility::All);
            }
            if file_label_idx.is_some() {
                self.ctrl.paths_navigator.select_label_idx(file_label_idx);
                let meta_data = self.ctrl.meta_data(
                    self.ctrl.file_selected_idx,
                    Some(self.ctrl.flags().is_loading_screen_active),
                );
                self.world.data.meta_data = meta_data;
                for t in &mut self.tools {
                    if t.is_active() {
                        (self.world, self.history) = t
                            .file_changed(mem::take(&mut self.world), mem::take(&mut self.history));
                    }
                }
            }
        }

        if are_tools_active(&self.menu, &self.tools_select_menu) {
            let meta_data = self.ctrl.meta_data(
                self.ctrl.file_selected_idx,
                Some(self.ctrl.flags().is_loading_screen_active),
            );
            self.world.data.meta_data = meta_data;
            (self.world, self.history) = apply_tools(
                &mut self.tools,
                mem::take(&mut self.world),
                mem::take(&mut self.history),
                e,
            );
        }

        // show position and rgb value
        if let Some(idx) = self.ctrl.paths_navigator.file_label_selected_idx() {
            let pixel_pos = e.mouse_pos_on_orig.map(|mp| mp.into());
            let data_point = get_pixel_on_orig_str(&self.world, &pixel_pos);
            let shape = self.world.shape_orig();
            let file_label = self.ctrl.file_label(idx);
            let active_tool = self.tools.iter().find(|t| t.is_active());
            let tool_string = if let Some(t) = active_tool {
                format!("{} tool is active", t.name)
            } else {
                "".to_string()
            };
            let s = match data_point {
                Some(s) => ImageInfo {
                    filename: file_label.to_string(),
                    shape_info: format!("{}x{}", shape.w, shape.h),
                    pixel_value: s,
                    tool_info: tool_string,
                },
                None => ImageInfo {
                    filename: file_label.to_string(),
                    shape_info: format!("{}x{}", shape.w, shape.h),
                    pixel_value: "(x, y) -> (r, g, b)".to_string(),
                    tool_info: tool_string,
                },
            };
            self.world.update_view.image_info = Some(s);
        }

        if let Some(n_autosaves) = self.ctrl.cfg.n_autosaves {
            if self.autosave_timer.elapsed().as_secs() > AUTOSAVE_INTERVAL_S {
                self.autosave_timer = Instant::now();

                let homefolder = self.ctrl.cfg.home_folder().map(PathBuf::from);
                let make_filepath = move |n| {
                    trace_ok(
                        homefolder
                            .clone()
                            .map(|hf| hf.join(format!("autosave_{n}.json", n = n))),
                    )
                };
                let mf_th = make_filepath.clone();
                thread::spawn(move || {
                    for i in 1..(n_autosaves) {
                        if let (Some(from), Some(to)) = (mf_th(i), mf_th(i - 1)) {
                            if from.exists() {
                                trace_ok(fs::copy(from, to));
                            }
                        }
                    }
                });
                let prj_path = make_filepath(n_autosaves - 1);
                if let Some(prj_path) = prj_path {
                    if trace_ok(
                        self.ctrl
                            .save(prj_path, &self.world.data.tools_data_map, false),
                    )
                    .is_some()
                    {
                        info!("autosaved");
                    }
                }
            }
        }

        Ok(mem::take(&mut self.world.update_view))
    }
    pub fn load_prj(&mut self, file_path: Option<PathBuf>) -> RvResult<()> {
        if let Some(file_path) = file_path {
            info!("loaded project {file_path:?}");
            self.world.data.tools_data_map = self.ctrl.load(file_path)?;
        } else {
            let pp = self.ctrl.cfg.current_prj_path().to_path_buf();
            // load last project
            match self.ctrl.load(pp) {
                Ok(td) => {
                    info!(
                        "loaded last saved project {:?}",
                        self.ctrl.cfg.current_prj_path()
                    );
                    self.world.data.tools_data_map = td;
                }
                Err(e) => {
                    if DEFAULT_PRJ_PATH.as_os_str() != self.ctrl.cfg.current_prj_path().as_os_str()
                    {
                        info!(
                            "could not read last saved project {:?} due to {e:?} ",
                            self.ctrl.cfg.current_prj_path()
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
