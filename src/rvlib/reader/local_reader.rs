use walkdir::WalkDir;

use crate::{
    cache::ReadImageToCache,
    result::{to_rv, RvError, RvResult},
    types::ResultImage,
    util,
};

use super::core::{CloneDummy, PickFolder, Picked, SUPPORTED_EXTENSIONS};

fn read_image_paths(path: &str) -> RvResult<Vec<String>> {
    WalkDir::new(path)
        .into_iter()
        .map(|p| p.map_err(to_rv))
        .filter(|p| match p {
            Err(_) => true,
            Ok(p_) => match p_.path().extension() {
                Some(ext) => SUPPORTED_EXTENSIONS
                    .iter()
                    .any(|sup_ext| Some(&sup_ext[1..]) == ext.to_str()),
                None => false,
            },
        })
        .map(|p| Ok(util::path_to_str(p?.path())?.to_string()))
        .collect::<RvResult<Vec<String>>>()
}
pub struct FileDialogPicker;
impl PickFolder for FileDialogPicker {
    fn pick() -> RvResult<Picked> {
        let sf = rfd::FileDialog::new()
            .pick_folder()
            .ok_or_else(|| RvError::new("Could not pick folder."))?;
        let path_as_string: String = sf
            .to_str()
            .ok_or_else(|| RvError::new("could not transfer path to unicode string"))?
            .to_string();
        let image_paths = read_image_paths(&path_as_string)?;
        Ok(Picked {
            folder_path: path_as_string,
            file_paths: image_paths,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ReadImageFromPath;
impl ReadImageToCache<CloneDummy> for ReadImageFromPath {
    fn new(_: CloneDummy) -> RvResult<Self> {
        Ok(Self {})
    }
    fn read(&self, path: &str) -> ResultImage {
        util::read_image(path)
    }
}

#[cfg(test)]
use {crate::format_rverr, std::env};
#[cfg(test)]
const TMP_SUBFOLDER: &str = "rvimage_testdata";
#[cfg(test)]
struct TmpFolderPicker;
#[cfg(test)]
impl PickFolder for TmpFolderPicker {
    fn pick() -> RvResult<Picked> {
        let tmpdir = env::temp_dir();
        Ok(Picked {
            folder_path: format!(
                "{}/{}",
                tmpdir
                    .to_str()
                    .ok_or_else(|| format_rverr!("cannot stringify {:?}", tmpdir))?,
                TMP_SUBFOLDER
            ),
            file_paths: vec![],
        })
    }
}
