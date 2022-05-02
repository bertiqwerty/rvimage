use std::path::Path;

use lazy_static::lazy_static;

use super::core::PickFolder;
use crate::{
    cache::ReadImageToCache,
    cfg,
    reader::core::{to_name_str, ReadImageFromPath},
    result::{to_rv, RvResult},
    ssh,
    util::{self, filename_in_tmpdir},
};

pub struct SshConfigPicker;
impl PickFolder for SshConfigPicker {
    fn pick() -> RvResult<(String, Vec<String>)> {
        let cfg = cfg::get_cfg()?;
        let folder = cfg.ssh_cfg.remote_folder_path.replace(' ', r"\ ");
        let ssh_cfg = cfg::get_cfg()?.ssh_cfg;
        let image_paths = ssh::ssh_ls(&ssh_cfg, &[".png", ".jpg"])?;
        Ok((folder, image_paths))
    }
}

pub struct ReadImageFromSsh;
impl ReadImageToCache for ReadImageFromSsh {
    fn read(remote_file_path: &str) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
        lazy_static! {
            pub static ref CFG: cfg::Cfg = cfg::get_cfg().unwrap();
        };
        let remote_file_path_path = Path::new(remote_file_path);
        let relative_file_name = if util::is_relative(remote_file_path) {
            remote_file_path
        } else {
            to_name_str(remote_file_path_path)?
        };
        let tmpdir = CFG.tmpdir()?;
        let local_file_path = filename_in_tmpdir(relative_file_name, tmpdir)?;
        if !Path::new(&local_file_path).exists() {
            let image_byte_blob = ssh::download(remote_file_path, &CFG.ssh_cfg)?;
            Ok(image::load_from_memory(&image_byte_blob)
                .map_err(to_rv)?
                .into_rgb8())
        } else {
            ReadImageFromPath::read(&local_file_path)
        }
    }
}
