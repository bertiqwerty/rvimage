use std::path::Path;

use lazy_static::lazy_static;

use super::core::PickFolder;
use crate::{
    cache::ImageReaderFn,
    cfg,
    reader::core::{path_to_str, ReadImageFromPath},
    result::RvResult,
    ssh,
};

pub struct SshConfigPicker;
impl PickFolder for SshConfigPicker {
    fn pick() -> RvResult<(String, Vec<String>)> {
        let cfg = cfg::get_cfg()?;
        let folder = cfg.ssh_cfg.remote_folder_path.replace(" ", r"\ ");
        let ssh_cfg = cfg::get_cfg()?.ssh_cfg;
        let image_paths = ssh::ssh_ls(folder.as_str(), &ssh_cfg)?;
        Ok((folder, image_paths))
    }
}

pub struct ReadImageFromSsh;
impl ImageReaderFn for ReadImageFromSsh {
    fn read(remote_file_name: &str) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
        lazy_static! {
            pub static ref CFG: cfg::Cfg = cfg::get_cfg().unwrap();
        };
        let scp_cfg = &CFG.ssh_cfg;
        let tmpdir = CFG.tmpdir()?;
        let local_file_path_tmp = Path::new(&tmpdir).join(&remote_file_name);
        let local_file_path = path_to_str(&local_file_path_tmp)?;
        let override_local = false;
        ssh::copy(remote_file_name, local_file_path, &scp_cfg, override_local)?;
        ReadImageFromPath::read(&local_file_path)
    }
}
