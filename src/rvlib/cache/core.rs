use crate::{
    result::RvResult,
    types::{AsyncResultImage, ResultImage},
};

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
        files: &[String],
        reload: bool,
    ) -> AsyncResultImage;
    fn ls(&self, folder_path: &str) -> RvResult<Vec<String>>;
    fn new(args: A) -> RvResult<Self>
    where
        Self: Sized;
}
