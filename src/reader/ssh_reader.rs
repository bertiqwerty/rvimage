use std::path::Path;

use lazy_static::lazy_static;

use super::core::PickFolder;
use crate::{
    cache::ImageReaderFn,
    cfg,
    reader::core::{path_to_str, to_name_str, ReadImageFromPath},
    result::RvResult,
    ssh, util,
};

pub struct SshConfigPicker;
impl PickFolder for SshConfigPicker {
    fn pick() -> RvResult<(String, Vec<String>)> {
        let cfg = cfg::get_cfg()?;
        let folder = cfg.ssh_cfg.remote_folder_path.replace(' ', r"\ ");
        let ssh_cfg = cfg::get_cfg()?.ssh_cfg;
        let image_paths = ssh::ssh_ls(folder.as_str(), &ssh_cfg, &[".png", ".jpg"])?;
        Ok((folder, image_paths))
    }
}

pub struct ReadImageFromSsh;
impl ImageReaderFn for ReadImageFromSsh {
    fn read(remote_file_name: &str) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
        lazy_static! {
            pub static ref CFG: cfg::Cfg = cfg::get_cfg().unwrap();
        };
        let remote_file_path = Path::new(remote_file_name);
        let rel = if util::is_relative(remote_file_name) {
            remote_file_name
        } else {
            to_name_str(remote_file_path)?
        };
        let tmpdir = CFG.tmpdir()?;
        let local_file_path_tmp = Path::new(&tmpdir).join(rel);
        let local_file_path = path_to_str(&local_file_path_tmp)?;
        let override_local = false;
        ssh::copy(
            remote_file_name,
            local_file_path,
            &CFG.ssh_cfg,
            override_local,
        )?;
        ReadImageFromPath::read(local_file_path)
    }
}
