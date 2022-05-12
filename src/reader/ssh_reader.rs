
use ssh2::Session;

use super::core::{PickFolder, Picked, SUPPORTED_EXTENSIONS};
use crate::{
    cache::ReadImageToCache,
    cfg::{self, SshCfg},
    result::{to_rv, ResultImage, RvResult},
    ssh,
};

pub struct SshConfigPicker;
impl PickFolder for SshConfigPicker {
    fn pick() -> RvResult<Picked> {
        let cfg = cfg::get_cfg()?;
        let folder = cfg.ssh_cfg.remote_folder_path;
        let ssh_cfg = cfg::get_cfg()?.ssh_cfg;
        let image_paths = ssh::find(&ssh_cfg, &SUPPORTED_EXTENSIONS)?;
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
}
impl ReadImageToCache<SshCfg> for ReadImageFromSsh {
    fn new(ssh_cfg: SshCfg) -> RvResult<Self> {
        Ok(Self {
            sess: ssh::auth(&ssh_cfg)?,
        })
    }
    fn read(&self, remote_file_path: &str) -> ResultImage {
        let image_byte_blob = ssh::download(remote_file_path, &self.sess)?;
        Ok(image::load_from_memory(&image_byte_blob)
            .map_err(to_rv)?
            .into_rgb8())
    }
}
