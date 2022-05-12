use std::path::Path;

use ssh2::Session;

use super::core::{PickFolder, Picked};
use crate::{
    cache::ReadImageToCache,
    cfg::{self, SshCfg},
    reader::{
        core::{to_name_str, CloneDummy},
        local_reader::ReadImageFromPath,
    },
    result::{to_rv, ResultImage, RvResult},
    ssh,
    util::{self, filename_in_tmpdir},
};

pub struct SshConfigPicker;
impl PickFolder for SshConfigPicker {
    fn pick() -> RvResult<Picked> {
        let cfg = cfg::get_cfg()?;
        let folder = cfg.ssh_cfg.remote_folder_path;
        let ssh_cfg = cfg::get_cfg()?.ssh_cfg;
        let image_paths = ssh::find(&ssh_cfg, &[".png", ".jpg"])?;
        Ok(Picked {
            folder_path: folder,
            file_paths: image_paths,
        })
    }
}

#[derive(Clone)]
pub struct ReadImageFromSshArgs {
    pub ssh_cfg: SshCfg,
    pub tmpdir: String,
}
#[derive(Clone)]
pub struct ReadImageFromSsh {
    sess: Session,
    tmpdir: String,
}
impl ReadImageToCache<ReadImageFromSshArgs> for ReadImageFromSsh {
    fn new(args: ReadImageFromSshArgs) -> RvResult<Self> {
        Ok(Self {
            sess: ssh::auth(&args.ssh_cfg)?,
            tmpdir: args.tmpdir
        })
    }
    fn read(&self, remote_file_path: &str) -> ResultImage {
        let remote_file_path_path = Path::new(remote_file_path);
        let relative_file_name = if util::is_relative(remote_file_path) {
            remote_file_path
        } else {
            to_name_str(remote_file_path_path)?
        };
        let local_file_path = filename_in_tmpdir(relative_file_name, &self.tmpdir)?;
        if !Path::new(&local_file_path).exists() {
            let image_byte_blob = ssh::download(remote_file_path, &self.sess)?;
            Ok(image::load_from_memory(&image_byte_blob)
                .map_err(to_rv)?
                .into_rgb8())
        } else {
            ReadImageFromPath::new(CloneDummy {})?.read(&local_file_path)
        }
    }
}
