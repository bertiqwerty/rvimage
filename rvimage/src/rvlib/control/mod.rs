use crate::cfg::{get_log_folder, Connection, ExportPath, ExportPathConnection, PyHttpReaderCfg};
use crate::file_util::{
    osstr_to_str, to_stem_str, PathPair, SavedCfg, DEFAULT_HOMEDIR, DEFAULT_PRJ_NAME,
    DEFAULT_PRJ_PATH,
};
use crate::history::{History, Record};
use crate::meta_data::{ConnectionData, MetaData, MetaDataFlags};
use crate::result::{trace_ok_err, trace_ok_warn};
use crate::sort_params::SortParams;
use crate::tools::{BBOX_NAME, BRUSH_NAME};
use crate::tools_data::set_tools_specific_data;
use crate::tools_data::{coco_io::read_coco, ToolSpecifics, ToolsDataMap};
use crate::world::World;
use crate::{
    cfg::Cfg, image_reader::ReaderFromCfg, threadpool::ThreadPool, types::AsyncResultImage,
};
use crate::{defer_file_removal, measure_time};
use chrono::{DateTime, Utc};
use detail::{create_lock_file, lock_file_path, read_user_from_lockfile};
use egui::ahash::HashSet;
use rvimage_domain::{rverr, to_rv, RvError, RvResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::{fs, mem};
use zip::write::ExtendedFileOptions;
mod filter;
pub mod paths_navigator;
use crate::image_reader::LoadImageForGui;
use paths_navigator::PathsNavigator;
use walkdir::WalkDir;

mod detail {
    use std::{
        mem,
        path::{Path, PathBuf},
    };

    use image::{DynamicImage, GenericImage};
    use imageproc::drawing::Canvas;
    use serde::{Deserialize, Serialize, Serializer};

    use crate::{
        cfg::{Cfg, CfgPrj},
        control::SavePrjData,
        defer_file_removal,
        file_util::{self, tf_to_annomap_key, SavedCfg, DEFAULT_HOMEDIR},
        result::trace_ok_err,
        tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME, ROT90_NAME},
        tools_data::{merge, ToolsDataMap},
        toolsdata_by_name,
        util::version_label,
        world::World,
    };
    use rvimage_domain::ShapeI;
    use rvimage_domain::{result::RvResult, to_rv};

    use super::UserPrjOpened;

    pub fn serialize_opened_folder<S>(
        folder: &Option<String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let cfg = trace_ok_err(Cfg::read(&DEFAULT_HOMEDIR));
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
        let cfg = trace_ok_err(Cfg::read(&DEFAULT_HOMEDIR));
        let prj_path = cfg.as_ref().map(|cfg| cfg.current_prj_path());
        let folder: Option<String> = Option::deserialize(deserializer)?;

        Ok(folder.map(|p| tf_to_annomap_key(p, prj_path)))
    }

    pub(super) fn lock_file_path(file_path: &Path) -> RvResult<PathBuf> {
        let stem = file_util::osstr_to_str(file_path.file_stem()).map_err(to_rv)?;
        Ok(file_path.with_file_name(format!(".{stem}_lock.json")))
    }
    pub(super) fn create_lock_file(file_path: &Path) -> RvResult<()> {
        let lock_file = lock_file_path(file_path)?;
        tracing::info!("creating lock file {lock_file:?}");
        let upo = UserPrjOpened::new();
        file_util::save(&lock_file, upo)
    }
    pub(super) fn remove_lock_file(prj_file_path: &Path) -> RvResult<()> {
        let lock_file = lock_file_path(prj_file_path)?;
        if lock_file.exists() {
            tracing::info!("removing lock file {lock_file:?}");
            defer_file_removal!(&lock_file);
        }
        Ok(())
    }
    pub(super) fn read_user_from_lockfile(prj_file_path: &Path) -> RvResult<Option<UserPrjOpened>> {
        let lock_file = lock_file_path(prj_file_path)?;
        let lock_file_content = file_util::read_to_string(lock_file).ok();
        lock_file_content
            .map(|lfc| serde_json::from_str(&lfc).map_err(to_rv))
            .transpose()
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
        trace_ok_err(cfg.write());
        let make_data = |tdm: &ToolsDataMap| SavePrjData {
            version: Some(version_label()),
            opened_folder: opened_folder.map(|of| of.to_string()),
            tools_data_map: tdm.clone(),
            cfg: SavedCfg::CfgPrj(cfg.prj.clone()),
        };
        tracing::info!("saved to {file_path:?}");
        write(tools_data_map, make_data, file_path)?;
        Ok(())
    }

    pub(super) fn draw_loading_dots(im: &mut DynamicImage, counter: u128) {
        let shape = ShapeI::from_im(im);
        let radius = 7u32;
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
        for (c_idx, ctr) in centers.iter().enumerate() {
            for y in ctr.1.saturating_sub(radius)..ctr.1.saturating_add(radius) {
                for x in ctr.0.saturating_sub(radius)..ctr.0.saturating_add(radius) {
                    let ctr0_x = x.abs_diff(ctr.0);
                    let ctr1_y = y.abs_diff(ctr.1);
                    let ctr0_x_sq = ctr0_x.saturating_mul(ctr0_x);
                    let ctr1_y_sq = ctr1_y.saturating_mul(ctr1_y);
                    if ctr0_x_sq + ctr1_y_sq < radius.pow(2) {
                        let counter_mod = ((counter / 5) % 3) as usize;
                        let rgb = off_center_dim(c_idx, counter_mod, &[195u8, 255u8, 205u8]);
                        let mut pixel = im.get_pixel(x, y);
                        pixel.0 = [rgb[0], rgb[1], rgb[2], 255];
                        im.put_pixel(x, y, pixel);
                    }
                }
            }
        }
    }
    pub(super) fn load(file_path: &Path) -> RvResult<(ToolsDataMap, Option<String>, CfgPrj)> {
        let s = file_util::read_to_string(file_path)?;

        let save_data = serde_json::from_str::<SavePrjData>(s.as_str()).map_err(to_rv)?;
        let cfg_prj = match save_data.cfg {
            SavedCfg::CfgLegacy(cfg) => cfg.to_cfg().prj,
            SavedCfg::CfgPrj(cfg_prj) => cfg_prj,
        };
        Ok((save_data.tools_data_map, save_data.opened_folder, cfg_prj))
    }

    #[derive(PartialEq)]
    enum FillResult {
        FilledCurWithLoaded,
        LoadedEmpty,
        BothNotEmpty,
        BothEmpty,
    }
    fn fill_empty_curtdm(
        tool: &str,
        cur_tdm: &mut ToolsDataMap,
        loaded_tdm: &mut ToolsDataMap,
    ) -> FillResult {
        if !cur_tdm.contains_key(tool) && loaded_tdm.contains_key(tool) {
            cur_tdm.insert(tool.to_string(), loaded_tdm[tool].clone());
            FillResult::FilledCurWithLoaded
        } else if !loaded_tdm.contains_key(tool) {
            FillResult::LoadedEmpty
        } else if cur_tdm.contains_key(tool) {
            FillResult::BothNotEmpty
        } else {
            FillResult::BothEmpty
        }
    }
    pub fn import_annos(cur_tdm: &mut ToolsDataMap, file_path: &Path) -> RvResult<()> {
        let (mut loaded_tdm, _, _) = load(file_path)?;

        if fill_empty_curtdm(BBOX_NAME, cur_tdm, &mut loaded_tdm) == FillResult::BothNotEmpty {
            let cur_bbox = toolsdata_by_name!(BBOX_NAME, bbox_mut, cur_tdm);
            let loaded_bbox = toolsdata_by_name!(BBOX_NAME, bbox_mut, loaded_tdm);
            let cur_annos = mem::take(&mut cur_bbox.annotations_map);
            let cur_li = mem::take(&mut cur_bbox.label_info);
            let loaded_annos = mem::take(&mut loaded_bbox.annotations_map);
            let loaded_li = mem::take(&mut loaded_bbox.label_info);
            let (merged_annos, merged_li) = merge(cur_annos, cur_li, loaded_annos, loaded_li);
            cur_bbox.annotations_map = merged_annos;
            cur_bbox.label_info = merged_li;
        }

        if fill_empty_curtdm(BRUSH_NAME, cur_tdm, &mut loaded_tdm) == FillResult::BothNotEmpty {
            let cur_brush = toolsdata_by_name!(BRUSH_NAME, brush_mut, cur_tdm);
            let loaded_brush = toolsdata_by_name!(BRUSH_NAME, brush_mut, loaded_tdm);
            let cur_annos = mem::take(&mut cur_brush.annotations_map);
            let cur_li = mem::take(&mut cur_brush.label_info);
            let loaded_annos = mem::take(&mut loaded_brush.annotations_map);
            let loaded_li = mem::take(&mut loaded_brush.label_info);
            let (merged_annos, merged_li) = merge(cur_annos, cur_li, loaded_annos, loaded_li);
            cur_brush.annotations_map = merged_annos;
            cur_brush.label_info = merged_li;
        }

        if fill_empty_curtdm(ROT90_NAME, cur_tdm, &mut loaded_tdm) == FillResult::BothNotEmpty {
            let cur_rot90 = toolsdata_by_name!(ROT90_NAME, rot90_mut, cur_tdm);
            let loaded_rot90 = toolsdata_by_name!(ROT90_NAME, rot90_mut, loaded_tdm);
            *cur_rot90 = mem::take(cur_rot90).merge(mem::take(loaded_rot90));
        }

        if fill_empty_curtdm(ATTRIBUTES_NAME, cur_tdm, &mut loaded_tdm) == FillResult::BothNotEmpty
        {
            let cur_attr = toolsdata_by_name!(ATTRIBUTES_NAME, attributes_mut, cur_tdm);
            let loaded_attr = toolsdata_by_name!(ATTRIBUTES_NAME, attributes_mut, loaded_tdm);
            *cur_attr = mem::take(cur_attr).merge(mem::take(loaded_attr));
        }
        Ok(())
    }
}
const LOAD_ACTOR_NAME: &str = "Load";

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct UserPrjOpened {
    time: DateTime<Utc>,
    username: String,
    realname: String,
}
impl UserPrjOpened {
    pub fn new() -> Self {
        UserPrjOpened {
            time: Utc::now(),
            username: whoami::username(),
            realname: whoami::realname(),
        }
    }
}
impl Display for UserPrjOpened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = format!(
            "{}-{}_{}",
            self.username,
            self.realname,
            self.time.format("%y%m%d-%H%M%S")
        );
        f.write_str(&s)
    }
}
impl Default for UserPrjOpened {
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
        tracing::info!("reload");
        if let Some(reader) = &mut self.reader {
            reader.clear_cache()?;
        }
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
        if let Some(label_selected) = label_selected {
            self.paths_navigator
                .select_file_label(label_selected.as_str());
        } else {
            self.file_selected_idx = None;
        }
        Ok(())
    }

    pub fn replace_with_save(&mut self, input_prj_path: &Path) -> RvResult<ToolsDataMap> {
        tracing::info!("replacing annotations with save from {input_prj_path:?}");
        let cur_prj_path = self.cfg.current_prj_path().to_path_buf();
        if let (Some(ifp_parent), Some(cpp_parent)) =
            (input_prj_path.parent(), cur_prj_path.parent())
        {
            let loaded = if ifp_parent != cpp_parent {
                // we need projects to be in the same folder for the correct resolution of relative paths
                let copied_file_path = cpp_parent.join(
                    input_prj_path
                        .file_name()
                        .ok_or_else(|| rverr!("could not get filename to copy to"))?,
                );
                defer_file_removal!(&copied_file_path);
                trace_ok_err(fs::copy(input_prj_path, &copied_file_path));
                let (tdm, _, _) = detail::load(input_prj_path)?;
                tdm
            } else {
                // are in the same parent folder, i.e., we replace with the last manual save
                let (tdm, _, _) = detail::load(input_prj_path)?;
                tdm
            };
            self.set_current_prj_path(cur_prj_path)?;
            self.cfg.write()?;
            Ok(loaded)
        } else {
            Err(rverr!("{cur_prj_path:?} does not have a parent folder"))
        }
    }
    pub fn load(&mut self, prj_path: PathBuf) -> RvResult<ToolsDataMap> {
        tracing::info!("loading project from {prj_path:?}");

        // check if project is already opened by someone
        let lockusr = read_user_from_lockfile(&prj_path)?;
        if let Some(lockusr) = lockusr {
            let usr = UserPrjOpened::new();
            if usr.username != lockusr.username || usr.realname != lockusr.realname {
                let lock_file_path = lock_file_path(&prj_path)?;
                let err = rverr!(
                    "The project is opened by {} ({}). Delete {:?} to unlock.",
                    lockusr.username,
                    lockusr.realname,
                    lock_file_path
                );
                Err(err)
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }?;

        // we need the project path before reading the annotations to map
        // their path correctly
        self.set_current_prj_path(prj_path.clone())?;
        self.cfg.write()?;
        let (tools_data_map, to_be_opened_folder, read_cfg) =
            detail::load(&prj_path).inspect_err(|_| {
                self.cfg.unset_current_prj_path();
                trace_ok_err(self.cfg.write());
            })?;
        if let Some(of) = to_be_opened_folder {
            self.open_relative_folder(of)?;
        }
        self.cfg.prj = read_cfg;
        // save cfg of loaded project
        trace_ok_err(self.cfg.write());
        Ok(tools_data_map)
    }

    fn wait_for_save(&mut self) {
        if self.save_handle.is_some() {
            mem::take(&mut self.save_handle).map(|h| trace_ok_err(h.join().map_err(to_rv)));
        }
    }
    pub fn import_annos(&self, prj_path: &Path, tools_data_map: &mut ToolsDataMap) -> RvResult<()> {
        tracing::info!("importing annotations from {prj_path:?}");
        detail::import_annos(tools_data_map, prj_path)
    }
    pub fn import_settings(&mut self, prj_path: &Path) -> RvResult<()> {
        tracing::info!("importing settings from {prj_path:?}");
        let (_, opened_folder, prj_cfg) = detail::load(prj_path)?;

        self.cfg.prj = prj_cfg;
        let info = UserPrjOpened::new();
        let filename = format!("{}_{info}_imported.rvi", to_stem_str(prj_path)?);
        let prj_path_imported = prj_path
            .parent()
            .ok_or_else(|| rverr!("prj path needs parent folder"))?
            .join(filename);
        self.cfg.set_current_prj_path(prj_path_imported);
        if let Some(of) = opened_folder {
            self.open_relative_folder(of)?;
        }
        Ok(())
    }
    pub fn import_both(
        &mut self,
        prj_path: &Path,
        tools_data_map: &mut ToolsDataMap,
    ) -> RvResult<()> {
        self.import_annos(prj_path, tools_data_map)?;
        self.import_settings(prj_path)?;
        Ok(())
    }
    pub fn import_from_coco(
        &mut self,
        coco_path: &str,
        tools_data_map: &mut ToolsDataMap,
        connection: ExportPathConnection,
    ) -> RvResult<()> {
        tracing::info!("importing from coco {coco_path:?}");

        let meta_data = self.meta_data(None, None);
        let path = ExportPath {
            path: Path::new(coco_path).to_path_buf(),
            conn: connection,
        };
        let (bbox_tool_data, brush_tool_data) = read_coco(&meta_data, &path, None)?;
        let server_addresses = bbox_tool_data
            .annotations_map
            .keys()
            .chain(brush_tool_data.annotations_map.keys())
            .filter(|k| k.starts_with("http://"))
            .flat_map(|k| k.rsplitn(2, '/').last())
            .collect::<HashSet<_>>();
        if !server_addresses.is_empty() {
            self.cfg.prj.connection = Connection::PyHttp;

            let server_addresses = server_addresses
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            self.cfg.prj.py_http_reader_cfg = Some(PyHttpReaderCfg { server_addresses });
        }
        let first_sa = server_addresses.iter().next().map(|s| s.to_string());
        if let Some(sa) = first_sa {
            self.open_relative_folder(sa.to_string())?;
        }

        set_tools_specific_data(
            tools_data_map,
            BRUSH_NAME,
            ToolSpecifics::Brush(brush_tool_data),
        );
        set_tools_specific_data(
            tools_data_map,
            BBOX_NAME,
            ToolSpecifics::Bbox(bbox_tool_data),
        );
        Ok(())
    }

    fn set_current_prj_path(&mut self, prj_path: PathBuf) -> RvResult<()> {
        trace_ok_warn(detail::create_lock_file(&prj_path));
        if prj_path != self.cfg.current_prj_path() {
            trace_ok_warn(detail::remove_lock_file(self.cfg.current_prj_path()));
        }
        self.cfg.set_current_prj_path(prj_path);
        Ok(())
    }

    pub fn save(
        &mut self,
        prj_path: PathBuf,
        tools_data_map: &ToolsDataMap,
        set_cur_prj: bool,
    ) -> RvResult<()> {
        tracing::info!("saving project to {prj_path:?}");
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
            self.set_current_prj_path(path.clone())?;
            // update prj name in cfg
            trace_ok_err(self.cfg.write());
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
        let cfg = Cfg::read(&DEFAULT_HOMEDIR).unwrap_or_else(|e| {
            tracing::warn!("could not read cfg due to {e:?}, returning default");
            Cfg::default()
        });
        if cfg.current_prj_path().exists() {
            trace_ok_warn(detail::create_lock_file(cfg.current_prj_path()));
        }
        trace_ok_warn(create_lock_file(cfg.current_prj_path()));
        let mut tmp = Self::default();
        tmp.cfg = cfg;
        tmp
    }
    pub fn new_prj(&mut self) -> ToolsDataMap {
        let mut cfg = Cfg::read(&DEFAULT_HOMEDIR).unwrap_or_else(|e| {
            tracing::warn!("could not read cfg due to {e:?}, returning default");
            Cfg::default()
        });
        trace_ok_warn(detail::remove_lock_file(self.cfg.current_prj_path()));
        cfg.unset_current_prj_path();
        *self = Control::default();
        self.cfg = cfg;
        HashMap::new()
    }

    pub fn reader(&self) -> Option<&ReaderFromCfg> {
        self.reader.as_ref()
    }

    pub fn read_image(&mut self, file_label_selected_idx: usize) -> AsyncResultImage {
        let wrapped_image = self.reader.as_mut().and_then(|r| {
            self.paths_navigator.paths_selector().as_ref().map(|ps| {
                let ffp = ps.filtered_abs_file_paths();
                r.read_image(file_label_selected_idx, &ffp)
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

    pub fn remake_reader(&mut self) -> RvResult<()> {
        let cfg = self.cfg.clone();
        self.last_open_folder_job_id = Some(
            self.tp
                .apply(Box::new(move || ReaderFromCfg::from_cfg(cfg)))?,
        );
        Ok(())
    }

    pub fn export_logs(&self, dst: &Path) -> RvResult<()> {
        let homefolder = self.cfg.home_folder();
        let log_folder = get_log_folder(Path::new(homefolder));
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
        Ok(())
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
            .map(|cfg| cfg.home_folder().to_string());
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
        measure_time!("load image if new", {
            let menu_file_selected = measure_time!("before if", {
                self.paths_navigator.file_label_selected_idx()
            });
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
                    let im_read = self.read_image(*selected)?;
                    let read_image_and_idx = match (abs_file_path, im_read) {
                        (Some(fp), Some(ri)) => {
                            tracing::info!("loading {} from {}", ri.info, fp);
                            self.file_selected_idx = menu_file_selected;
                            self.file_info_selected = Some(ri.info);
                            let mut new_world = world.clone();
                            new_world.set_background_image(ri.im);
                            new_world.reset_updateview();

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

                            tracing::debug!("still loading...");
                            self.file_selected_idx = menu_file_selected;
                            self.flags.is_loading_screen_active = true;
                            let mut new_world = world.clone();

                            detail::draw_loading_dots(
                                new_world.data.im_background_mut(),
                                self.loading_screen_animation_counter,
                            );
                            new_world.reset_updateview();
                            (new_world, self.file_selected_idx)
                        }
                    };
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
        })
    }
}

#[cfg(test)]
use {
    crate::{
        file_util::DEFAULT_TMPDIR,
        tools_data::{BboxToolData, ToolsData},
    },
    rvimage_domain::{make_test_bbs, ShapeI},
    std::str::FromStr,
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
            a.add_bb(bb, 0, crate::InstanceLabelDisplay::IndexLr);
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

impl Drop for Control {
    fn drop(&mut self) {
        trace_ok_warn(detail::remove_lock_file(self.cfg.current_prj_path()));
    }
}

#[test]
fn test_save_load() {
    let tdm = make_data(&PathBuf::from_str("dummyfile").unwrap());
    let cfg = {
        let mut tmp = Cfg::default();
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
