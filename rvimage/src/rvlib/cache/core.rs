use crate::types::{AsyncResultImage, ResultImage};

use rvimage_domain::RvResult;
pub trait ReadImageToCache<A> {
    fn read(&self, abs_path: &str) -> ResultImage;
    fn file_info(&self, abs_path: &str) -> RvResult<String>;
    fn ls(&self, abs_folder_path: &str) -> RvResult<Vec<String>>;
    fn new(args: A) -> RvResult<Self>
    where
        Self: Sized;
}

pub trait Cache<A> {
    fn load_from_cache(
        &mut self,
        selected_file_idx: usize,
        abs_file_paths: &[&str],
        reload: bool,
    ) -> AsyncResultImage;
    fn ls(&self, abs_folder_path: &str) -> RvResult<Vec<String>>;
    fn new(args: A) -> RvResult<Self>
    where
        Self: Sized;
}
