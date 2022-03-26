use crate::{
    cache::file_cache::FileCache,
    cfg,
    result::{to_rv, RvError, RvResult},
};
use std::path::PathBuf;
use std::str::FromStr;

use super::{PickFolder, ReadImageFiles};

pub struct ScpConfigPicker;
impl PickFolder for ScpConfigPicker {
    fn pick() -> RvResult<PathBuf> {
        let cfg = cfg::get_cfg()?;
        PathBuf::from_str(&cfg.scp_cfg.remote_folder_path).map_err(to_rv)
    }
}

pub struct ScpReader;

impl ReadImageFiles for ScpReader {
    fn new() -> Self {
        Self
    }
    fn next(&mut self) {}
    fn prev(&mut self) {}
    fn read_image(
        &mut self,
        file_selected_idx: usize,
    ) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
        Err(RvError::new("not implemented"))
    }
    fn open_folder(&mut self) -> RvResult<()> {
        Err(RvError::new("not implemented"))
    }
    fn file_selected_idx(&self) -> Option<usize> {
        None
    }
    fn selected_file(&mut self, idx: usize) {}
    fn list_file_labels(&self) -> RvResult<Vec<String>> {
        Err(RvError::new("not implemented"))
    }
    fn folder_label(&self) -> RvResult<String> {
        Err(RvError::new("not implemented"))
    }
    fn file_selected_label(&self) -> RvResult<String> {
        Err(RvError::new("not implemented"))
    }
}
