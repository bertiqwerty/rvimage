use walkdir::WalkDir;

use crate::{
    cache::ReadImageToCache,
    file_util, image_util,
    result::{to_rv, RvResult},
    types::ResultImage,
};

use super::core::{CloneDummy, SUPPORTED_EXTENSIONS};

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
        .map(|p| Ok(file_util::path_to_str(p?.path())?.to_string()))
        .collect::<RvResult<Vec<String>>>()
}

#[derive(Clone, Debug)]
pub struct ReadImageFromPath;
impl ReadImageToCache<CloneDummy> for ReadImageFromPath {
    fn new(_: CloneDummy) -> RvResult<Self> {
        Ok(Self {})
    }
    fn read(&self, path: &str) -> ResultImage {
        image_util::read_image(path)
    }
    fn ls(&self, folder_path: &str) -> RvResult<Vec<String>> {
        let image_paths = read_image_paths(folder_path)?;
        Ok(image_paths)
    }
    fn file_info(&self, path: &str) -> RvResult<String> {
        Ok(file_util::local_file_info(path))
    }
}
