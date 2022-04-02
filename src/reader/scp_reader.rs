use lazy_static::lazy_static;

use super::{local_reader::read_image_from_path, PickFolder, ReadImageFiles};
use crate::{
    cache::Preload,
    cfg::{self, get_cfg},
    result::{to_rv, RvError, RvResult},
    ssh::{self, copy},
};
use std::{path::PathBuf, str::FromStr};

pub struct ScpConfigPicker;
impl PickFolder for ScpConfigPicker {
    fn pick() -> RvResult<PathBuf> {
        let cfg = cfg::get_cfg()?;
        PathBuf::from_str(&cfg.ssh_cfg.remote_folder_path).map_err(to_rv)
    }
}

fn read_image_from_ssh(file_name: &str) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
    lazy_static! {
        pub static ref CFG: cfg::Cfg = get_cfg().unwrap();
    };
    let scp_cfg = &CFG.ssh_cfg;
    let tmpdir = CFG.tmpdir()?;
    let dst = copy(file_name, &tmpdir, &scp_cfg)?;
    read_image_from_path(&dst)
}

pub struct ScpReader<C>
where
    C: Preload,
{
    cache: RvResult<C>,
    files: Vec<String>
}
impl<C: Preload> ScpReader<C> {
    pub fn new() -> Self {
        Self {
            cache: Ok(C::new(|file_name| read_image_from_ssh(file_name))),
            files: vec![],
        }
    }
}
impl<C: Preload> ReadImageFiles for ScpReader<C> {
    fn open_folder(&mut self) -> RvResult<()> {
        let folder = ScpConfigPicker::pick()?;
        let ssh_cfg = get_cfg()?.ssh_cfg;
        self.files = ssh::ssh_ls(
            folder.to_str().ok_or_else(|| RvError::new("hmm?"))?,
            &ssh_cfg,
        )?;
        Ok(())
    }
    fn next(&mut self) {}
    fn prev(&mut self) {}
    fn read_image(
        &mut self,
        file_selected_idx: usize,
    ) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
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
