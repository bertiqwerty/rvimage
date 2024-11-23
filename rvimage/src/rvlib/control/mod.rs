use crate::cfg::{self, get_log_folder, Connection};
use crate::file_util::{
    self, osstr_to_str, PathPair, SavedCfg, DEFAULT_PRJ_NAME, DEFAULT_PRJ_PATH,
};
use crate::history::{History, Record};
use crate::meta_data::{ConnectionData, MetaData, MetaDataFlags};
use crate::result::trace_ok_err;
use crate::sort_params::SortParams;
use crate::world::{DataRaw, ToolsDataMap, World};
use crate::{
    cfg::Cfg, image_reader::ReaderFromCfg, threadpool::ThreadPool, types::AsyncResultImage,
};
use chrono::{DateTime, Utc};
use rvimage_domain::{to_rv, RvError, RvResult};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::fs;
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use zip::write::ExtendedFileOptions;
mod filter;
pub mod paths_navigator;
use crate::image_reader::LoadImageForGui;
use paths_navigator::PathsNavigator;
use tracing::{info, warn};
use walkdir::WalkDir;

mod detail {
    use std::path::Path;

    use image::{DynamicImage, ImageBuffer};
    use serde::{Deserialize, Serialize, Serializer};

    use crate::{
        cfg::{read_cfg, write_cfg, Cfg, CfgPrj},
        control::{LastTimePrjOpened, SavePrjData},
        file_util::{self, tf_to_annomap_key, SavedCfg},
        result::trace_ok_err,
        util::version_label,
        world::{ToolsDataMap, World},
    };
    use rvimage_domain::result::RvResult;
    use rvimage_domain::ShapeI;

    use super::load_prj;

    pub fn serialize_opened_folder<S>(
        folder: &Option<String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let cfg = trace_ok_err(read_cfg());
        let prj_path = cfg.as_ref().map(|cfg| cfg.current_prj_path());
        let folder = folder
            .clone()
            .map(|folder| tf_to_annomap_key(folder, prj_path));
        folder.serialize(serializer)
    }
    pub fn deserialize_opened_folder<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let cfg = trace_ok_err(read_cfg());
        let prj_path = cfg.as_ref().map(|cfg| cfg.current_prj_path());
        let folder: Option<String> = Option::deserialize(deserializer)?;

        Ok(folder.map(|p| tf_to_annomap_key(p, prj_path)))
    }

    pub(super) fn idx_change_check(
        file_selected_idx: Option<usize>,
        world_idx_pair: Option<(World, Option<usize>)>,
    ) -> Option<(World, Option<usize>)> {
        world_idx_pair.map(|(w, idx)| {
            if idx != file_selected_idx {
                (w, idx)
            } else {
                (w, None)
            }
        })
    }

    fn write<T>(
        tools_data_map: &ToolsDataMap,
        make_data: impl Fn(&ToolsDataMap) -> T,
        export_path: &Path,
    ) -> RvResult<()>
    where
        T: Serialize,
    {
        let tools_data_map = tools_data_map
            .iter()
            .map(|(k, v)| {
                let mut v = v.clone();
                v.menu_active = false;
                (k.clone(), v)
            })
            .collect::<ToolsDataMap>();
        let data = make_data(&tools_data_map);
        file_util::save(export_path, data)
    }

    pub fn save(
        opened_folder: Option<&str>,
        tools_data_map: &ToolsDataMap,
        file_path: &Path,
        cfg: &Cfg,
    ) -> RvResult<()> {
        // we need to write the cfg for correct prj-path mapping during serialization
        // of annotations
        trace_ok_err(write_cfg(cfg));
        let make_data = |tdm: &ToolsDataMap| SavePrjData {
            version: Some(version_label()),
            opened_folder: opened_folder.map(|of| of.to_string()),
            tools_data_map: tdm.clone(),
            cfg: SavedCfg::CfgPrj(cfg.prj.clone()),
            last_time_prj_opened: LastTimePrjOpened::new(),
        };
        tracing::info!("saved to {file_path:?}");
        write(tools_data_map, make_data, file_path)?;
        Ok(())
    }

    pub(super) fn loading_image(shape: ShapeI, counter: u128) -> DynamicImage {
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
                if (ctr.0 as i32 - x as i32).pow(2) + (ctr.1 as i32 - y as i32).pow(2)
                    < radius.pow(2)
                {
                    let counter_mod = ((counter / 5) % 3) as usize;
                    return image::Rgb(off_center_dim(c_idx, counter_mod, &[195u8, 255u8, 205u8]));
                }
            }
            image::Rgb([77u8, 77u8, 87u8])
        }))
    }
    pub(super) fn load(file_path: &Path) -> RvResult<(ToolsDataMap, Option<String>, CfgPrj)> {
        let save_data = load_prj(file_path)?;
        let cfg_prj = match save_data.cfg {
            SavedCfg::CfgLegacy(cfg) => cfg.to_cfg().prj,
            SavedCfg::CfgPrj(cfg_prj) => cfg_prj,
        };
        Ok((save_data.tools_data_map, save_data.opened_folder, cfg_prj))
    }
}
const LOAD_ACTOR_NAME: &str = "Load";

pub fn load_prj(file_path: &Path) -> RvResult<SavePrjData> {
    let s = file_util::read_to_string(file_path)?;
    serde_json::from_str::<SavePrjData>(s.as_str()).map_err(to_rv)
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct LastTimePrjOpened {
    time: DateTime<Utc>,
    username: String,
    realname: String,
}
impl LastTimePrjOpened {
    pub fn new() -> Self {
        LastTimePrjOpened {
            time: Utc::now(),
            username: whoami::username(),
            realname: whoami::realname(),
        }
    }
}
impl Default for LastTimePrjOpened {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct SavePrjData {
    pub version: Option<String>,
    #[serde(serialize_with = "detail::serialize_opened_folder")]
    #[serde(deserialize_with = "detail::deserialize_opened_folder")]
    pub opened_folder: Option<String>,
    pub tools_data_map: ToolsDataMap,
    pub cfg: SavedCfg,
    #[serde(default = "LastTimePrjOpened::new")]
    pub last_time_prj_opened: LastTimePrjOpened,
}

#[derive(Clone, Debug, Default)]
pub enum Info {
    Error(String),
    Warning(String),
    #[default]
    None,
}

#[derive(Default)]
pub struct ControlFlags {
    pub undo_redo_load: bool,
    pub is_loading_screen_active: bool,
    pub reload_cached_images: bool,
}

#[derive(Default)]
pub struct Control {
    pub reader: Option<ReaderFromCfg>,
    pub info: Info,
    pub paths_navigator: PathsNavigator,
    pub opened_folder: Option<PathPair>,
    tp: ThreadPool<RvResult<ReaderFromCfg>>,
    last_open_folder_job_id: Option<u128>,
    pub cfg: Cfg,
    pub file_loaded: Option<usize>,
    pub file_selected_idx: Option<usize>,
    pub file_info_selected: Option<String>,
    flags: ControlFlags,
    pub loading_screen_animation_counter: u128,
    pub log_export_path: Option<PathBuf>,
    save_handle: Option<JoinHandle<()>>,
}

impl Control {
    pub fn http_address(&self) -> String {
        self.cfg.http_address().to_string()
    }
    pub fn flags(&self) -> &ControlFlags {
        &self.flags
    }
    pub fn reload(&mut self, sort_params: Option<SortParams>) -> RvResult<()> {
        if let Some(sort_params) = sort_params {
            self.cfg.prj.sort_params = sort_params;
        }
        let label_selected = self.file_selected_idx.and_then(|idx| {
            self.paths_navigator.len_filtered().and_then(|len_f| {
                if idx < len_f {
                    Some(self.file_label(idx).to_string())
                } else {
                    None
                }
            })
        });
        self.load_opened_folder_content(self.cfg.prj.sort_params)?;
        self.flags.reload_cached_images = true;
        if let Some(label_selected) = label_selected {
            self.paths_navigator
                .select_file_label(label_selected.as_str());
        } else {
            self.file_selected_idx = None;
        }
        Ok(())
    }

    pub fn load(&mut self, file_path: PathBuf) -> RvResult<ToolsDataMap> {
        // we need the project path before reading the annotations to map
        // their path correctly
        self.cfg.set_current_prj_path(file_path.clone());
        cfg::write_cfg(&self.cfg)?;

        let (tools_data_map, to_be_opened_folder, read_cfg) = detail::load(&file_path)?;
        if let Some(of) = to_be_opened_folder {
            self.open_relative_folder(of)?;
        }
        self.cfg.prj = read_cfg;

        // save cfg of loaded project
        trace_ok_err(cfg::write_cfg(&self.cfg));

        // write time stamp to indicate that the project is currently open
        self.write_lasttimeprjopened();

        Ok(tools_data_map)
    }

    fn wait_for_save(&mut self) {
        if self.save_handle.is_some() {
            mem::take(&mut self.save_handle).map(|h| trace_ok_err(h.join().map_err(to_rv)));
        }
    }

    pub fn write_lasttimeprjopened(&mut self) {
        if self.save_handle.as_ref().map(|h| h.is_finished()) != Some(false) {
            self.wait_for_save(); // paranoia check
            let prj_path = self.cfg.current_prj_path().to_path_buf();
            let handle = thread::spawn(move || {
                tracing::info!("writing now as last time the project has been opened...");
                let prj_data = trace_ok_err(load_prj(&prj_path));
                if let Some(mut prj_data) = prj_data {
                    prj_data.last_time_prj_opened = LastTimePrjOpened::new();
                    trace_ok_err(file_util::save(&prj_path, prj_data));
                }
                tracing::info!("writing now as ... done!");
            });
            self.save_handle = Some(handle);
        }
    }

    pub fn save(
        &mut self,
        prj_path: PathBuf,
        tools_data_map: &ToolsDataMap,
        set_cur_prj: bool,
    ) -> RvResult<()> {
        let path = if let Some(of) = self.opened_folder() {
            if DEFAULT_PRJ_PATH.as_os_str() == prj_path.as_os_str() {
                PathBuf::from(of.path_relative()).join(DEFAULT_PRJ_NAME)
            } else {
                prj_path.clone()
            }
        } else {
            prj_path.clone()
        };

        if set_cur_prj {
            self.cfg.set_current_prj_path(path.clone());
            // update prj name in cfg
            trace_ok_err(cfg::write_cfg(&self.cfg));
        }
        let opened_folder = self.opened_folder().cloned();
        let tdm = tools_data_map.clone();
        let cfg = self.cfg.clone();
        self.wait_for_save();
        let handle = thread::spawn(move || {
            trace_ok_err(detail::save(
                opened_folder.as_ref().map(|of| of.path_relative()),
                &tdm,
                path.as_path(),
                &cfg,
            ));
        });
        self.save_handle = Some(handle);
        Ok(())
    }

    pub fn new() -> Self {
        let cfg = cfg::read_cfg().unwrap_or_else(|e| {
            warn!("could not read cfg due to {e:?}, returning default");
            cfg::get_default_cfg()
        });
        Self {
            cfg,
            ..Default::default()
        }
    }

    pub fn reader(&self) -> Option<&ReaderFromCfg> {
        self.reader.as_ref()
    }

    pub fn read_image(&mut self, file_label_selected_idx: usize, reload: bool) -> AsyncResultImage {
        let wrapped_image = self.reader.as_mut().and_then(|r| {
            self.paths_navigator.paths_selector().as_ref().map(|ps| {
                let ffp = ps.filtered_abs_file_paths();
                r.read_image(file_label_selected_idx, &ffp, reload)
            })
        });
        match wrapped_image {
            None => Ok(None),
            Some(x) => Ok(x?),
        }
    }

    fn make_reader(&mut self, cfg: Cfg) -> RvResult<()> {
        self.paths_navigator = PathsNavigator::new(None, SortParams::default())?;
        self.last_open_folder_job_id = Some(
            self.tp
                .apply(Box::new(move || ReaderFromCfg::from_cfg(cfg)))?,
        );
        Ok(())
    }

    pub fn export_logs(&self, dst: &Path) {
        if let Ok(log_folder) = get_log_folder() {
            tracing::info!("exporting logs from {log_folder:?} to {dst:?}");
            let elf = log_folder.clone();
            let dst = dst.to_path_buf();
            thread::spawn(move || {
                // zip log folder
                let mut zip = zip::ZipWriter::new(fs::File::create(&dst).unwrap());

                let walkdir = WalkDir::new(elf);
                let iter_log = walkdir.into_iter();
                for entry in iter_log {
                    if let Some(entry) = trace_ok_err(entry) {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = osstr_to_str(path.file_name());
                            trace_ok_err(file_name).and_then(|file_name| {
                                trace_ok_err(zip.start_file::<&str, ExtendedFileOptions>(
                                    file_name,
                                    zip::write::FileOptions::default(),
                                ));
                                trace_ok_err(fs::read(path))
                                    .and_then(|buf| trace_ok_err(zip.write_all(&buf)))
                            });
                        }
                    }
                }
            });
        } else {
            tracing::error!("could not get log folder");
        }
    }

    pub fn open_relative_folder(&mut self, new_folder: String) -> RvResult<()> {
        tracing::info!("new opened folder {new_folder}");
        self.make_reader(self.cfg.clone())?;
        let current_prj_path = match self.cfg.prj.connection {
            Connection::Local => Some(self.cfg.current_prj_path()),
            _ => None,
        };
        self.opened_folder = Some(PathPair::from_relative_path(new_folder, current_prj_path));
        Ok(())
    }

    pub fn load_opened_folder_content(&mut self, sort_params: SortParams) -> RvResult<()> {
        if let (Some(opened_folder), Some(reader)) = (&self.opened_folder, &self.reader) {
            let prj_folder = self.cfg.current_prj_path();
            let selector = reader.open_folder(opened_folder.path_absolute(), prj_folder)?;
            self.paths_navigator = PathsNavigator::new(Some(selector), sort_params)?;
        }
        Ok(())
    }

    pub fn check_if_connected(&mut self, sort_params: SortParams) -> RvResult<bool> {
        if let Some(job_id) = self.last_open_folder_job_id {
            let tp_res = self.tp.result(job_id);
            if let Some(res) = tp_res {
                self.last_open_folder_job_id = None;
                res.and_then(|reader| {
                    self.reader = Some(reader);
                    self.load_opened_folder_content(sort_params)?;
                    Ok(true)
                })
            } else {
                Ok(false)
            }
        } else {
            Ok(true)
        }
    }

    pub fn opened_folder_label(&self) -> Option<&str> {
        self.paths_navigator
            .paths_selector()
            .as_ref()
            .map(|ps| ps.folder_label())
    }

    pub fn file_label(&self, idx: usize) -> &str {
        match self.paths_navigator.paths_selector() {
            Some(ps) => ps.filtered_idx_file_label_pairs(idx).1,
            None => "",
        }
    }

    pub fn cfg_of_opened_folder(&self) -> Option<&Cfg> {
        self.reader().map(|r| r.cfg())
    }

    fn opened_folder(&self) -> Option<&PathPair> {
        self.opened_folder.as_ref()
    }

    pub fn connection_data(&self) -> RvResult<ConnectionData> {
        let cfg = self
            .cfg_of_opened_folder()
            .ok_or_else(|| RvError::new("save failed, open folder first"));
        Ok(match self.cfg.prj.connection {
            Connection::Ssh => {
                let ssh_cfg = cfg.map(|cfg| cfg.ssh_cfg())?;
                ConnectionData::Ssh(ssh_cfg)
            }
            Connection::Local => ConnectionData::None,
            Connection::PyHttp => {
                let pyhttp_cfg = cfg
                    .map(|cfg| cfg.prj.py_http_reader_cfg.clone())?
                    .ok_or_else(|| RvError::new("cannot open pyhttp without pyhttp cfg"))?;
                ConnectionData::PyHttp(pyhttp_cfg)
            }
            #[cfg(feature = "azure_blob")]
            Connection::AzureBlob => {
                let azure_blob_cfg = cfg
                    .map(|cfg| cfg.azure_blob_cfg())?
                    .ok_or_else(|| RvError::new("cannot open azure blob without cfg"))?;
                ConnectionData::AzureBlobCfg(azure_blob_cfg)
            }
        })
    }

    pub fn meta_data(
        &self,
        file_selected_idx: Option<usize>,
        is_loading_screen_active: Option<bool>,
    ) -> MetaData {
        let file_path =
            file_selected_idx.and_then(|fsidx| self.paths_navigator.file_path(fsidx).cloned());
        let open_folder = self.opened_folder().cloned();
        let connection_data = if self.reader.is_some() {
            ConnectionData::Ssh(self.cfg.ssh_cfg())
        } else {
            ConnectionData::None
        };
        let export_folder = self
            .cfg_of_opened_folder()
            .map(|cfg| cfg.home_folder().map(|ef| ef.to_string()).unwrap());
        let is_file_list_empty = Some(file_path.is_none());
        let prj_path = self.cfg.current_prj_path();
        MetaData::new(
            file_path,
            file_selected_idx,
            connection_data,
            Some(self.cfg.ssh_cfg()),
            open_folder,
            export_folder,
            MetaDataFlags {
                is_loading_screen_active,
                is_file_list_empty,
            },
            Some(prj_path.to_path_buf()),
        )
    }

    pub fn redo(&mut self, history: &mut History) -> Option<(World, Option<usize>)> {
        self.flags.undo_redo_load = true;
        detail::idx_change_check(
            self.file_selected_idx,
            history.next_world(&self.opened_folder),
        )
    }
    pub fn undo(&mut self, history: &mut History) -> Option<(World, Option<usize>)> {
        self.flags.undo_redo_load = true;
        detail::idx_change_check(
            self.file_selected_idx,
            history.prev_world(&self.opened_folder),
        )
    }

    pub fn load_new_image_if_triggered(
        &mut self,
        world: &World,
        history: &mut History,
    ) -> RvResult<Option<(World, Option<usize>)>> {
        let menu_file_selected = self.paths_navigator.file_label_selected_idx();
        let world_idx_pair = if self.file_selected_idx != menu_file_selected
            || self.flags.is_loading_screen_active
        {
            // load new image
            if let Some(selected) = &menu_file_selected {
                let abs_file_path = menu_file_selected.and_then(|fs| {
                    Some(
                        self.paths_navigator
                            .file_path(fs)?
                            .path_absolute()
                            .replace('\\', "/"),
                    )
                });
                let im_read = self.read_image(*selected, self.flags.reload_cached_images)?;
                let read_image_and_idx = match (abs_file_path, menu_file_selected, im_read) {
                    (Some(fp), Some(fidx), Some(ri)) => {
                        info!("loading {} from {}", ri.info, fp);
                        self.file_selected_idx = menu_file_selected;
                        self.file_info_selected = Some(ri.info);
                        let ims_raw = DataRaw::new(
                            ri.im,
                            world.data.tools_data_map.clone(),
                            MetaData::from_filepath(fp, fidx, self.cfg.current_prj_path()),
                            world.ui_image_rect(),
                        );
                        let zoom_box = if ims_raw.shape() == world.data.shape() {
                            *world.zoom_box()
                        } else {
                            None
                        };
                        let new_world = World::new(ims_raw, zoom_box);
                        if !self.flags.undo_redo_load {
                            history.push(Record {
                                world: world.clone(),
                                actor: LOAD_ACTOR_NAME,
                                file_label_idx: self.file_selected_idx,
                                opened_folder: self
                                    .opened_folder
                                    .as_ref()
                                    .map(|of| of.path_absolute().to_string()),
                            });
                        }
                        self.flags.undo_redo_load = false;
                        self.flags.is_loading_screen_active = false;
                        (new_world, self.file_selected_idx)
                    }
                    _ => {
                        thread::sleep(Duration::from_millis(2));
                        let shape = world.shape_orig();
                        self.file_selected_idx = menu_file_selected;
                        self.flags.is_loading_screen_active = true;
                        (
                            World::new(
                                DataRaw::new(
                                    detail::loading_image(
                                        shape,
                                        self.loading_screen_animation_counter,
                                    ),
                                    world.data.tools_data_map.clone(),
                                    MetaData::default(),
                                    world.ui_image_rect(),
                                ),
                                None,
                            ),
                            self.file_selected_idx,
                        )
                    }
                };
                self.flags.reload_cached_images = false;
                Some(read_image_and_idx)
            } else {
                None
            }
        } else {
            None
        };
        self.loading_screen_animation_counter += 1;
        if self.loading_screen_animation_counter == u128::MAX {
            self.loading_screen_animation_counter = 0;
        }
        Ok(world_idx_pair)
    }
}

#[cfg(test)]
use {
    crate::{
        defer_file_removal,
        file_util::DEFAULT_TMPDIR,
        tools::BBOX_NAME,
        tools_data::{BboxToolData, ToolSpecifics, ToolsData},
    },
    rvimage_domain::{make_test_bbs, ShapeI},
    std::{collections::HashMap, str::FromStr},
};
#[cfg(test)]
pub fn make_data(image_file: &Path) -> ToolsDataMap {
    use crate::tools_data::VisibleInactiveToolsState;

    let test_export_folder = DEFAULT_TMPDIR.clone();

    match fs::create_dir(&test_export_folder) {
        Ok(_) => (),
        Err(e) => {
            println!("{e:?}");
        }
    }

    let mut bbox_data = BboxToolData::new();
    bbox_data
        .label_info
        .push("x".to_string(), None, None)
        .unwrap();
    bbox_data
        .label_info
        .remove_catidx(0, &mut bbox_data.annotations_map);
    let mut bbs = make_test_bbs();
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());

    let annos = bbox_data.get_annos_mut(
        image_file.as_os_str().to_str().unwrap(),
        ShapeI::new(10, 10),
    );
    if let Some(a) = annos {
        for bb in bbs {
            a.add_bb(bb, 0);
        }
    }

    HashMap::from([(
        BBOX_NAME.to_string(),
        ToolsData::new(
            ToolSpecifics::Bbox(bbox_data),
            VisibleInactiveToolsState::default(),
        ),
    )])
}

#[test]
fn test_save_load() {
    let tdm = make_data(&PathBuf::from_str("dummyfile").unwrap());
    let cfg = {
        let mut tmp = cfg::get_default_cfg();
        tmp.usr.n_autosaves = Some(59);
        tmp
    };
    let opened_folder_name = "dummy_opened_folder";
    let export_folder = cfg.tmpdir();
    let export_file = PathBuf::new().join(export_folder).join("export.json");
    let opened_folder = Some(opened_folder_name.to_string());
    detail::save(opened_folder.as_deref(), &tdm, &export_file, &cfg).unwrap();

    defer_file_removal!(&export_file);

    let (tdm_imported, _, cfg_imported) = detail::load(&export_file).unwrap();
    assert_eq!(tdm, tdm_imported);
    assert_eq!(cfg.prj, cfg_imported);
}
