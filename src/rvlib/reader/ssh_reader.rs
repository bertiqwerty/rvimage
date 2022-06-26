use ssh2::Session;

use super::core::{ListFilesInFolder, SUPPORTED_EXTENSIONS};
use crate::{
    cache::ReadImageToCache,
    cfg::{self, SshCfg},
    result::{to_rv, RvResult},
    ssh,
    types::ResultImage,
};

pub struct SshLister;
impl ListFilesInFolder for SshLister {
    fn list(folder_path: &str) -> RvResult<Vec<String>> {
        let cfg = cfg::get_cfg()?;
        let sess = ssh::auth(&cfg.ssh_cfg)?;
        let image_paths = ssh::find(sess, folder_path, &SUPPORTED_EXTENSIONS)?;
        Ok(image_paths)
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
        image::load_from_memory(&image_byte_blob).map_err(to_rv)
    }
}
