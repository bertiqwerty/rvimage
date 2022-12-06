use std::fmt::Debug;
use std::path::PathBuf;

use crate::cfg::{get_default_cfg, Connection};
use crate::file_util::{ConnectionData, MetaData};
use crate::result::RvError;
use crate::world::ToolsDataMap;
use crate::{
    cfg::Cfg, reader::ReaderFromCfg, result::RvResult, threadpool::ThreadPool,
    types::AsyncResultImage,
};
pub mod paths_navigator;
use crate::reader::LoadImageForGui;
use paths_navigator::PathsNavigator;

mod detail {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use crate::{
        file_util::{self, get_last_part_of_path, ConnectionData, ExportData},
        result::{to_rv, RvResult},
        rverr,
        tools::BBOX_NAME,
        tools_data::{BboxExportData, BboxSpecificData, ToolSpecifics, ToolsData},
        world::ToolsDataMap,
    };

    pub(super) fn load(
        export_folder: &str,
        file_name: &str,
        mut tools_data_map: ToolsDataMap,
    ) -> RvResult<(ToolsDataMap, String, ConnectionData)> {
        let file_path = Path::new(export_folder).join(file_name);
        let s = file_util::read_to_string(file_path)?;
        let read: ExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;

        if let Some(bbox_data) = read.bbox_data {
            let bbox_data = BboxSpecificData::from_bbox_export_data(bbox_data)?;
            let tools_data = tools_data_map.get_mut(BBOX_NAME);
            if let Some(td) = tools_data {
                td.specifics = ToolSpecifics::Bbox(bbox_data);
            } else {
                tools_data_map.insert(BBOX_NAME, ToolsData::new(ToolSpecifics::Bbox(bbox_data)));
            }
        }
        Ok((tools_data_map, read.opened_folder, read.connection_data))
    }

    pub fn save(
        opened_folder: Option<&String>,
        tools_data_map: &ToolsDataMap,
        connection_data: ConnectionData,
        export_folder: &str,
    ) -> RvResult<Option<PathBuf>> {
        let mut res = None;
        let bbox_data = tools_data_map.get(BBOX_NAME);
        if let (Some(of), Some(bbox_data)) = (opened_folder, bbox_data) {
            let bbox_data = bbox_data.clone();
            let data = ExportData {
                opened_folder: of.to_string(),
                bbox_data: Some(BboxExportData::from_bbox_data(
                    bbox_data.specifics.bbox().clone(),
                )),
                connection_data,
            };

            let of_last_part = get_last_part_of_path(of)
                .map(|lp| lp.name())
                .unwrap_or_else(|| of.to_string());
            let ef_path = Path::new(export_folder);
            match fs::create_dir_all(ef_path) {
                Ok(_) => Ok(()),
                Err(e) => Err(rverr!("could not create {:?} due to {:?}", ef_path, e)),
            }?;
            let path = Path::new(ef_path).join(of_last_part).with_extension("json");
            let data_str = serde_json::to_string(&data).map_err(to_rv)?;
            file_util::write(&path, data_str)?;
            println!("saved to {:?}", path);
            res = Some(path);
        } else {
            println!("did not save");
            println!("  opened folder {:?}", opened_folder);
            println!("  bbox data {:?}", bbox_data);
        }
        Ok(res)
    }
}

#[derive(Clone, Debug, Default)]
pub enum Info {
    Error(String),
    Warning(String),
    #[default]
    None,
}

#[derive(Default)]
pub struct Control {
    pub reader: Option<ReaderFromCfg>,
    pub info: Info,
    pub paths_navigator: PathsNavigator,
    pub opened_folder: Option<String>,
    tp: ThreadPool<(ReaderFromCfg, Info)>,
    last_open_folder_job_id: Option<u128>,
    pub cfg: Cfg,
    pub file_loaded: Option<usize>,
}

impl Control {
    pub fn load(
        &mut self,
        file_name: &str,
        mut tools_data_map: ToolsDataMap,
    ) -> RvResult<ToolsDataMap> {
        let export_folder = &self.cfg.export_folder()?;
        let loaded = detail::load(export_folder, file_name, tools_data_map)?;
        tools_data_map = loaded.0;
        let to_be_opened_folder = loaded.1;
        let connection_data = loaded.2;

        match connection_data {
            ConnectionData::Ssh(ssh_cfg) => {
                self.cfg.ssh_cfg = ssh_cfg;
                self.cfg.connection = Connection::Ssh;
            }
            ConnectionData::PyHttp(pyhttp_cfg) => {
                self.cfg.py_http_reader_cfg = Some(pyhttp_cfg);
                self.cfg.connection = Connection::PyHttp;
            }
            #[cfg(feature = "azure_blob")]
            ConnectionData::AzureBlobCfg(azure_blob_cfg) => {
                self.cfg.azure_blob_cfg = Some(azure_blob_cfg);
                self.cfg.connection = Connection::AzureBlob;
            }
            ConnectionData::None => {
                self.cfg.connection = Connection::Local;
            }
        }
        self.open_folder(to_be_opened_folder)?;

        Ok(tools_data_map)
    }

    pub fn save(&self, tools_data_map: &ToolsDataMap) -> RvResult<Option<PathBuf>> {
        let opened_folder = self.opened_folder();
        let connection_data = self.connection_data();
        let export_folder = self.cfg.export_folder()?;

        detail::save(
            opened_folder,
            tools_data_map,
            connection_data,
            export_folder,
        )
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
        self.paths_navigator = PathsNavigator::new(None);
        self.last_open_folder_job_id =
            Some(self.tp.apply(Box::new(move || make_reader_from_cfg(cfg)))?);
        Ok(())
    }

    pub fn open_folder(&mut self, new_folder: String) -> RvResult<()> {
        println!("new opened folder {}", new_folder);
        self.make_reader(self.cfg.clone())?;
        self.opened_folder = Some(new_folder);
        Ok(())
    }

    pub fn load_opened_folder_content(&mut self) -> RvResult<()> {
        if let (Some(opened_folder), Some(reader)) = (&self.opened_folder, &self.reader) {
            let selector = reader.open_folder(opened_folder.as_str())?;
            self.paths_navigator = PathsNavigator::new(Some(selector));
        }
        Ok(())
    }

    pub fn check_if_connected(&mut self) -> RvResult<bool> {
        if let Some(job_id) = self.last_open_folder_job_id {
            let tp_res = self.tp.result(job_id);
            if let Some((reader, info)) = tp_res {
                self.last_open_folder_job_id = None;
                self.reader = Some(reader);
                self.load_opened_folder_content()?;
                self.info = info;
                Ok(true)
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
            Some(ps) => ps.file_labels()[idx].1.as_str(),
            None => "",
        }
    }

    pub fn cfg_of_opened_folder(&self) -> Option<&Cfg> {
        self.reader().map(|r| r.cfg())
    }

    fn opened_folder(&self) -> Option<&String> {
        self.opened_folder.as_ref()
    }

    pub fn connection_data(&self) -> ConnectionData {
        match self.cfg.connection {
            Connection::Ssh => {
                let ssh_cfg = self
                    .cfg_of_opened_folder()
                    .map(|cfg| cfg.ssh_cfg.clone())
                    .ok_or_else(|| RvError::new("save failed, opened folder needs a config"))
                    .unwrap();
                ConnectionData::Ssh(ssh_cfg)
            }
            Connection::Local => ConnectionData::None,
            Connection::PyHttp => {
                let pyhttp_cfg = self
                    .cfg_of_opened_folder()
                    .map(|cfg| cfg.py_http_reader_cfg.clone())
                    .ok_or_else(|| RvError::new("save failed, opened folder needs a config"))
                    .unwrap()
                    .ok_or_else(|| RvError::new("cannot open pyhttp without pyhttp cfg"))
                    .unwrap();
                ConnectionData::PyHttp(pyhttp_cfg)
            }
            #[cfg(feature = "azure_blob")]
            Connection::AzureBlob => {
                let azure_blob_cfg = self
                    .cfg_of_opened_folder()
                    .map(|cfg| cfg.azure_blob_cfg.clone())
                    .ok_or_else(|| RvError::new("save failed, opened folder needs a config"))
                    .unwrap()
                    .ok_or_else(|| RvError::new("cannot open azure blob without cfg"))
                    .unwrap();
                ConnectionData::AzureBlobCfg(azure_blob_cfg)
            }
        }
    }

    pub fn meta_data(&self, file_selected: Option<usize>) -> MetaData {
        let file_path =
            file_selected.and_then(|fs| self.paths_navigator.file_path(fs).map(|s| s.to_string()));
        let open_folder = self.opened_folder().cloned();
        let ssh_cfg = self.cfg_of_opened_folder().map(|cfg| cfg.ssh_cfg.clone());
        let connection_data = match ssh_cfg {
            Some(ssh_cfg) => ConnectionData::Ssh(ssh_cfg),
            None => ConnectionData::None,
        };
        let export_folder = self
            .cfg_of_opened_folder()
            .map(|cfg| cfg.export_folder().map(|ef| ef.to_string()).unwrap());
        MetaData {
            file_path,
            connection_data,
            opened_folder: open_folder,
            export_folder,
        }
    }
}

fn make_reader_from_cfg(cfg: Cfg) -> (ReaderFromCfg, Info) {
    match ReaderFromCfg::from_cfg(cfg) {
        Ok(rfc) => (rfc, Info::None),
        Err(e) => (
            ReaderFromCfg::from_cfg(get_default_cfg()).expect("default cfg broken"),
            Info::Warning(e.msg().to_string()),
        ),
    }
}

#[cfg(test)]
use {
    crate::{
        cfg, defer_file_removal,
        domain::{make_test_bbs, Shape},
        file_util::DEFAULT_TMPDIR,
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
            println!("{:?}", e);
        }
    }

    let mut bbox_data = BboxSpecificData::new();
    bbox_data.push("x".to_string(), None, None).unwrap();
    bbox_data.remove_catidx(0);
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
    for bb in bbs {
        annos.add_bb(bb, 0);
    }
    let tools_data_map =
        HashMap::from([(BBOX_NAME, ToolsData::new(ToolSpecifics::Bbox(bbox_data)))]);
    tools_data_map
}

#[test]
fn test_save_load() {
    let tdm = make_data(&PathBuf::from_str("dummyfile").unwrap());
    let cfg = cfg::get_default_cfg();

    let opened_folder_name = "dummy_opened_folder";
    let export_folder = cfg.tmpdir().unwrap();
    let opened_folder = Some(opened_folder_name.to_string());
    let path = detail::save(
        opened_folder.as_ref(),
        &tdm,
        ConnectionData::None,
        export_folder,
    )
    .unwrap()
    .unwrap();

    defer_file_removal!(&path);

    let (tdm_imported, _, _) = detail::load(
        export_folder,
        format!("{}.json", opened_folder_name).as_str(),
        tdm.clone(),
    )
    .unwrap();
    assert_eq!(tdm, tdm_imported);
}
