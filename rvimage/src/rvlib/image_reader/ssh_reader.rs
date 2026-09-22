use std::path::Path;

use ssh2::Session;

use super::core::SUPPORTED_EXTENSIONS;
use crate::{
    cache::{ImageUploader, ReadImageToCache},
    cfg::SshCfg,
    image_reader::core::upload_via_bytes,
    ssh,
    types::ResultImage,
};
use rvimage_domain::{RvResult, to_rv};

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

    fn ls(&self, folder_path: &str) -> RvResult<Vec<String>> {
        let image_paths = ssh::find(folder_path, &SUPPORTED_EXTENSIONS, &self.sess)?;
        Ok(image_paths)
    }

    fn file_info(&self, path: &str) -> RvResult<String> {
        ssh::file_info(path, &self.sess)
    }
    fn make_uploader(&self) -> ImageUploader {
        let sess = self.sess.clone();
        Box::new(
            move |src_file: &Path, target_folder: &str| -> RvResult<()> {
                upload_via_bytes(src_file, target_folder, |buffer, target_path| {
                    ssh::write_bytes(&buffer, Path::new(target_path), &sess)
                })
            },
        )
    }
}
