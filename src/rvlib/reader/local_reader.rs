use walkdir::WalkDir;

use crate::{
    cache::ReadImageToCache,
    result::{to_rv, RvResult},
    types::ResultImage,
    util,
};

use super::core::{CloneDummy, ListFilesInFolder, SUPPORTED_EXTENSIONS};

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
pub struct LocalLister;
impl ListFilesInFolder for LocalLister {
    fn list(folder_path: &str) -> RvResult<Vec<String>> {
        let image_paths = read_image_paths(folder_path)?;
        Ok(image_paths)
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
