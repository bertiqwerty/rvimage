use crate::{
    cache::{file_cache::FileCache, Preload},
    cfg,
    result::{to_rv, RvError, RvResult},
};
use std::str::FromStr;
use std::{marker::PhantomData, path::PathBuf};

use super::{PickFolder, ReadImageFiles};

pub struct ScpConfigPicker;
impl PickFolder for ScpConfigPicker {
    fn pick() -> RvResult<PathBuf> {
        let cfg = cfg::get_cfg()?;
        PathBuf::from_str(&cfg.scp_cfg.remote_folder_path).map_err(to_rv)
    }
}

pub struct ScpReader<C: Preload> {
    pick_phantom: PhantomData<C>,
}
impl<C: Preload> ScpReader<C> {
    pub fn new() -> Self {
        Self {pick_phantom: PhantomData{}}
    }
}
impl<C: Preload> ReadImageFiles for ScpReader<C> {
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
    fn select_file(&mut self, idx: usize) {}
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
