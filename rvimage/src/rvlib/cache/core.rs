use crate::types::{AsyncResultImage, ResultImage};

use rvimage_domain::RvResult;
pub trait ReadImageToCache<A> {
    fn read(&self, path: &str) -> ResultImage;
    fn file_info(&self, path: &str) -> RvResult<String>;
    fn ls(&self, folder_path: &str) -> RvResult<Vec<String>>;
    fn new(args: A) -> RvResult<Self>
    where
        Self: Sized;
}

pub trait Cache<A> {
    fn load_from_cache(
        &mut self,
        selected_file_idx: usize,
        files: &[&str],
        reload: bool,
    ) -> AsyncResultImage;
    fn ls(&self, folder_path: &str) -> RvResult<Vec<String>>;
    fn new(args: A) -> RvResult<Self>
    where
        Self: Sized;
}
