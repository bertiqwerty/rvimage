use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use crate::file_util::{ConnectionData, ExportData};
use crate::result::to_rv;
use crate::tools::BBOX_NAME;
use crate::tools_data::{ToolSpecifics, ToolsData};
use crate::world::ToolsDataMap;
use crate::{
    cfg::Cfg, reader::ReaderFromCfg, result::RvResult, threadpool::ThreadPool,
    types::AsyncResultImage,
};
pub mod paths_navigator;
use crate::reader::LoadImageForGui;
use paths_navigator::PathsNavigator;

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
}
impl Control {
    pub fn import<P>(&mut self, filename: P, tools_data_map: &mut ToolsDataMap) -> RvResult<()>
    where
        P: AsRef<Path>,
    {
        if filename.as_ref().extension() == Some(OsStr::new("json")) {
            let s = fs::read_to_string(filename).map_err(to_rv)?;
            let read: ExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
            match read.connection_data {
                ConnectionData::Ssh(ssh_cfg) => {
                    self.cfg.ssh_cfg = ssh_cfg;
                }
                ConnectionData::None => (),
            }
            self.open_folder(read.opened_folder)?;
            if let Some(bbox_data) = read.bbox_data {
                let bbox_data = bbox_data.to_bbox_data()?;
                let tools_data = tools_data_map.get_mut(BBOX_NAME);
                if let Some(td) = tools_data {
                    td.specifics = ToolSpecifics::Bbox(bbox_data);
                } else {
                    tools_data_map
                        .insert(BBOX_NAME, ToolsData::new(ToolSpecifics::Bbox(bbox_data)));
                }
            }
        }
        Ok(())
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
}
fn make_reader_from_cfg(cfg: Cfg) -> (ReaderFromCfg, Info) {
    match ReaderFromCfg::from_cfg(cfg) {
        Ok(rfc) => (rfc, Info::None),
        Err(e) => (
            ReaderFromCfg::new().expect("default cfg broken"),
            Info::Warning(e.msg().to_string()),
        ),
    }
}
