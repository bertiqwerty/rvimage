use lazy_static::lazy_static;

use super::core::PickFolder;
use crate::{
    cache::ImageReaderFn,
    cfg::{self, get_cfg},
    reader::core::ReadImageFromPath,
    result::RvResult,
    ssh::{self, copy},
};

pub struct SshConfigPicker;
impl PickFolder for SshConfigPicker {
    fn pick() -> RvResult<(String, Vec<String>)> {
        let cfg = cfg::get_cfg()?;
        let folder = cfg.ssh_cfg.remote_folder_path;
        let ssh_cfg = get_cfg()?.ssh_cfg;
        let image_paths = ssh::ssh_ls(folder.as_str(), &ssh_cfg)?;
        Ok((folder, image_paths))
    }
}

pub struct ReadImageFromSsh;
impl ImageReaderFn for ReadImageFromSsh {
    fn read(file_name: &str) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
        lazy_static! {
            pub static ref CFG: cfg::Cfg = get_cfg().unwrap();
        };
        let scp_cfg = &CFG.ssh_cfg;
        let tmpdir = CFG.tmpdir()?;
        let dst = copy(file_name, &tmpdir, &scp_cfg)?;
        ReadImageFromPath::read(&dst)
    }
}
