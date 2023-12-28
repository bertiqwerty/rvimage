use std::fmt::Debug;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::cfg::{self, Connection};
use crate::file_util::{filename_to_prjname, ConnectionData, MetaData};
use crate::history::{History, Record};
use crate::result::RvError;
use crate::world::{DataRaw, ToolsDataMap, World};
use crate::{
    cfg::Cfg, image_reader::ReaderFromCfg, result::RvResult, threadpool::ThreadPool,
    types::AsyncResultImage,
};
mod filter;
pub mod paths_navigator;
use crate::image_reader::LoadImageForGui;
use paths_navigator::PathsNavigator;

mod detail {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use image::{DynamicImage, ImageBuffer};
    use tracing::info;

    use crate::{
        cfg::Cfg,
        domain::Shape,
        file_util::{self, make_prjcfg_path, ExportData, ExportDataLegacy, DEFAULT_PRJ_NAME},
        result::{to_rv, RvResult},
        rverr,
        tools::BBOX_NAME,
        tools_data::{BboxSpecificData, ToolSpecifics, ToolsData},
        util::version_label,
        world::ToolsDataMap,
    };

    pub(super) fn load(
        export_folder: &str,
        file_name: &str,
    ) -> RvResult<(ToolsDataMap, Option<String>, Cfg)> {
        let file_path = Path::new(export_folder).join(file_name);
        let s = file_util::read_to_string(file_path)?;
        let (tools_data_map, cfg, opened_folder) =
            match serde_json::from_str::<ExportData>(s.as_str()).map_err(to_rv) {
                Ok(export_data) => (
                    export_data.tools_data_map,
                    export_data.cfg,
                    export_data.opened_folder,
                ),
                Err(e) => {
                    info!("trying legacy-read on {file_name:?} due to {e:?}");
                    let read =
                        serde_json::from_str::<ExportDataLegacy>(s.as_str()).map_err(to_rv)?;
                    let tdm = if let Some(bbox_data) = read.bbox_data {
                        let mut bbox_data = BboxSpecificData::from_bbox_export_data(bbox_data)?;

                        if let Some(options) = read.bbox_options {
                            bbox_data.options = options;
                        }
                        ToolsDataMap::from([(
                            BBOX_NAME.to_string(),
                            ToolsData::new(ToolSpecifics::Bbox(bbox_data)),
                        )])
                    } else {
                        ToolsDataMap::new()
                    };
                    (tdm, read.cfg, read.opened_folder)
                }
            };

        Ok((tools_data_map, opened_folder, cfg))
    }

    pub fn save(
        opened_folder: Option<&String>,
        tools_data_map: &ToolsDataMap,
        export_folder: &str,
        cfg: &Cfg,
    ) -> RvResult<PathBuf> {
        let tools_data_map = tools_data_map
            .iter()
            .map(|(k, v)| {
                let mut v = v.clone();
                v.menu_active = false;
                (k.clone(), v)
            })
            .collect::<ToolsDataMap>();
        let data = ExportData {
            version: Some(version_label()),
            opened_folder: opened_folder.cloned(),
            tools_data_map: tools_data_map.clone(),
            cfg: cfg.clone(),
        };
        let prj_name = if DEFAULT_PRJ_NAME != cfg.current_prj_name {
            &cfg.current_prj_name
        } else if let Some(of) = opened_folder {
            of.as_str()
        } else {
            DEFAULT_PRJ_NAME
        };
        let ef_path = Path::new(export_folder);
        let path = make_prjcfg_path(ef_path, prj_name);
        match fs::create_dir_all(ef_path) {
            Ok(_) => Ok(()),
            Err(e) => Err(rverr!("could not create {:?} due to {:?}", ef_path, e)),
        }?;
        let data_str = serde_json::to_string(&data).map_err(to_rv)?;
        file_util::write(&path, data_str)?;
        tracing::info!("saved to {path:?}");
        Ok(path)
    }
    pub(super) fn loading_image(shape: Shape, counter: u128) -> DynamicImage {
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
}
const LOAD_ACTOR_NAME: &str = "Load";

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

#[derive(Default, PartialEq, Debug, Clone, Copy)]
pub enum SortType {
    #[default]
    Natural,
    Alphabetical,
}

#[derive(Default)]
pub struct Control {
    pub reader: Option<ReaderFromCfg>,
    pub info: Info,
    pub paths_navigator: PathsNavigator,
    pub opened_folder: Option<String>,
    tp: ThreadPool<RvResult<ReaderFromCfg>>,
    last_open_folder_job_id: Option<u128>,
    pub cfg: Cfg,
    pub file_loaded: Option<usize>,
    pub file_selected_idx: Option<usize>,
    pub file_info_selected: Option<String>,
    flags: ControlFlags,
    pub loading_screen_animation_counter: u128,
}

impl Control {
    pub fn flags(&self) -> &ControlFlags {
        &self.flags
    }
    pub fn reload(&mut self, sort_type: SortType) -> RvResult<()> {
        let label_selected = self.file_selected_idx.and_then(|idx| {
            self.paths_navigator.len_filtered().and_then(|len_f| {
                if idx < len_f {
                    Some(self.file_label(idx).to_string())
                } else {
                    None
                }
            })
        });
        self.load_opened_folder_content(sort_type)?;
        self.flags.reload_cached_images = true;
        if let Some(label_selected) = label_selected {
            self.paths_navigator
                .select_file_label(label_selected.as_str());
        } else {
            self.file_selected_idx = None;
        }
        Ok(())
    }
    pub fn load(&mut self, file_name: &str) -> RvResult<ToolsDataMap> {
        let export_folder = &self.cfg.export_folder()?;
        let (tools_data_map, to_be_opened_folder, read_cfg) =
            detail::load(export_folder, file_name)?;
        if let Some(of) = to_be_opened_folder {
            self.open_folder(of)?;
        }
        self.cfg = read_cfg;

        // update prj name in cfg
        let mut cfg_global = cfg::get_cfg()?;
        cfg_global.current_prj_name = filename_to_prjname(file_name)?.to_string();
        cfg::write_cfg(&cfg_global)?;

        Ok(tools_data_map)
    }

    pub fn sort(
        &mut self,
        sort_type: SortType,
        filter_str: &str,
        tools_data_map: &ToolsDataMap,
        active_tool_name: Option<&str>,
    ) -> RvResult<()> {
        match sort_type {
            SortType::Alphabetical => {
                self.paths_navigator
                    .alphabetical_sort(filter_str, tools_data_map, active_tool_name)
            }
            SortType::Natural => {
                self.paths_navigator
                    .natural_sort(filter_str, tools_data_map, active_tool_name)
            }
        }
    }

    pub fn save(&self, tools_data_map: &ToolsDataMap) -> RvResult<PathBuf> {
        let opened_folder = self.opened_folder();
        let export_folder = self.cfg.export_folder()?;

        detail::save(opened_folder, tools_data_map, export_folder, &self.cfg)
    }

    pub fn new(cfg: Cfg) -> Self {
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
                let ffp = ps.filtered_file_paths();
                r.read_image(file_label_selected_idx, &ffp, reload)
            })
        });
        match wrapped_image {
            None => Ok(None),
            Some(x) => Ok(x?),
        }
    }

    fn make_reader(&mut self, cfg: Cfg) -> RvResult<()> {
        self.paths_navigator = PathsNavigator::new(None, SortType::default())?;
        self.last_open_folder_job_id = Some(
            self.tp
                .apply(Box::new(move || ReaderFromCfg::from_cfg(cfg)))?,
        );
        Ok(())
    }

    pub fn open_folder(&mut self, new_folder: String) -> RvResult<()> {
        tracing::info!("new opened folder {new_folder}");
        self.make_reader(self.cfg.clone())?;
        self.opened_folder = Some(new_folder);
        Ok(())
    }

    pub fn load_opened_folder_content(&mut self, sort_type: SortType) -> RvResult<()> {
        if let (Some(opened_folder), Some(reader)) = (&self.opened_folder, &self.reader) {
            let selector = reader.open_folder(opened_folder.as_str())?;
            self.paths_navigator = PathsNavigator::new(Some(selector), sort_type)?;
        }
        Ok(())
    }

    pub fn check_if_connected(&mut self, sort_type: SortType) -> RvResult<bool> {
        if let Some(job_id) = self.last_open_folder_job_id {
            let tp_res = self.tp.result(job_id);
            if let Some(res) = tp_res {
                self.last_open_folder_job_id = None;
                res.and_then(|reader| {
                    self.reader = Some(reader);
                    self.load_opened_folder_content(sort_type)?;
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

    fn opened_folder(&self) -> Option<&String> {
        self.opened_folder.as_ref()
    }

    pub fn connection_data(&self) -> RvResult<ConnectionData> {
        let cfg = self
            .cfg_of_opened_folder()
            .ok_or_else(|| RvError::new("save failed, open folder first"));
        Ok(match self.cfg.connection {
            Connection::Ssh => {
                let ssh_cfg = cfg.map(|cfg| cfg.ssh_cfg.clone())?;
                ConnectionData::Ssh(ssh_cfg)
            }
            Connection::Local => ConnectionData::None,
            Connection::PyHttp => {
                let pyhttp_cfg = cfg
                    .map(|cfg| cfg.py_http_reader_cfg.clone())?
                    .ok_or_else(|| RvError::new("cannot open pyhttp without pyhttp cfg"))?;
                ConnectionData::PyHttp(pyhttp_cfg)
            }
            #[cfg(feature = "azure_blob")]
            Connection::AzureBlob => {
                let azure_blob_cfg = cfg
                    .map(|cfg| cfg.azure_blob_cfg.clone())?
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
        let file_path = file_selected_idx
            .and_then(|fsidx| self.paths_navigator.file_path(fsidx).map(|s| s.to_string()));
        let open_folder = self.opened_folder().cloned();
        let ssh_cfg = self.cfg_of_opened_folder().map(|cfg| cfg.ssh_cfg.clone());
        let connection_data = match &ssh_cfg {
            Some(ssh_cfg) => ConnectionData::Ssh(ssh_cfg.clone()),
            None => ConnectionData::None,
        };
        let export_folder = self
            .cfg_of_opened_folder()
            .map(|cfg| cfg.export_folder().map(|ef| ef.to_string()).unwrap());
        MetaData {
            file_path,
            connection_data,
            ssh_cfg,
            opened_folder: open_folder,
            export_folder,
            is_loading_screen_active,
        }
    }

    fn make_folder_label(&self) -> Option<String> {
        self.paths_navigator.folder_label().map(|s| s.to_string())
    }
    pub fn redo(&mut self, history: &mut History) -> Option<(DataRaw, Option<usize>)> {
        self.flags.undo_redo_load = true;
        history.next_world(&self.make_folder_label())
    }
    pub fn undo(&mut self, history: &mut History) -> Option<(DataRaw, Option<usize>)> {
        self.flags.undo_redo_load = true;
        history.prev_world(&self.make_folder_label())
    }

    pub fn load_new_image_if_triggered(
        &mut self,
        world: &mut World,
        history: &mut History,
    ) -> RvResult<Option<(DataRaw, Option<usize>)>> {
        let menu_file_selected = self.paths_navigator.file_label_selected_idx();

        let ims_raw_idx_pair = if self.file_selected_idx != menu_file_selected
            || self.flags.is_loading_screen_active
        {
            // load new image
            if let Some(selected) = &menu_file_selected {
                let folder_label = self.make_folder_label();
                let file_path = menu_file_selected
                    .and_then(|fs| Some(self.paths_navigator.file_path(fs)?.to_string()));
                let im_read = self.read_image(*selected, self.flags.reload_cached_images)?;
                let read_image_and_idx = match (file_path, im_read) {
                    (Some(fp), Some(ri)) => {
                        self.file_info_selected = Some(ri.info);
                        let ims_raw = DataRaw::new(
                            ri.im,
                            MetaData::from_filepath(fp),
                            world.data.tools_data_map.clone(),
                        );
                        if !self.flags.undo_redo_load {
                            history.push(Record {
                                data: ims_raw.clone(),
                                actor: LOAD_ACTOR_NAME,
                                file_label_idx: self.file_selected_idx,
                                folder_label,
                            });
                        }
                        self.flags.undo_redo_load = false;
                        self.file_selected_idx = menu_file_selected;
                        self.flags.is_loading_screen_active = false;
                        (ims_raw, self.file_selected_idx)
                    }
                    _ => {
                        thread::sleep(Duration::from_millis(20));
                        let shape = world.shape_orig();
                        self.file_selected_idx = menu_file_selected;
                        self.flags.is_loading_screen_active = true;
                        (
                            DataRaw::new(
                                detail::loading_image(shape, self.loading_screen_animation_counter),
                                MetaData::default(),
                                world.data.tools_data_map.clone(),
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
        Ok(ims_raw_idx_pair)
    }
}

#[cfg(test)]
use {
    crate::{
        defer_file_removal,
        domain::{make_test_bbs, Shape},
        file_util::{make_prjcfg_filename, DEFAULT_TMPDIR},
        tools::BBOX_NAME,
        tools_data::{BboxSpecificData, ToolSpecifics, ToolsData},
    },
    std::{collections::HashMap, fs, path::Path, str::FromStr},
};
#[cfg(test)]
pub fn make_data(image_file: &Path) -> ToolsDataMap {
    let test_export_folder = DEFAULT_TMPDIR.clone();

    match fs::create_dir(&test_export_folder) {
        Ok(_) => (),
        Err(e) => {
            println!("{e:?}");
        }
    }

    let mut bbox_data = BboxSpecificData::new();
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

    let annos =
        bbox_data.get_annos_mut(image_file.as_os_str().to_str().unwrap(), Shape::new(10, 10));
    if let Some(a) = annos {
        for bb in bbs {
            a.add_bb(bb, 0);
        }
    }
    let tools_data_map = HashMap::from([(
        BBOX_NAME.to_string(),
        ToolsData::new(ToolSpecifics::Bbox(bbox_data)),
    )]);
    tools_data_map
}

#[test]
fn test_save_load() {
    let tdm = make_data(&PathBuf::from_str("dummyfile").unwrap());
    let cfg = cfg::get_default_cfg();
    let opened_folder_name = "dummy_opened_folder";
    let export_folder = cfg.tmpdir().unwrap();
    let opened_folder = Some(opened_folder_name.to_string());
    let path = detail::save(opened_folder.as_ref(), &tdm, export_folder, &cfg).unwrap();

    defer_file_removal!(&path);

    let (tdm_imported, _, cfg_imported) = detail::load(
        export_folder,
        &make_prjcfg_filename(opened_folder_name).as_str(),
    )
    .unwrap();
    assert_eq!(tdm, tdm_imported);
    assert_eq!(cfg, cfg_imported);
}
