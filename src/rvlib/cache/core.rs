use crate::{
    result::RvResult,
    types::{AsyncResultImage, ResultImage},
};

pub trait ReadImageToCache<A> {
    fn read(&self, path: &str) -> ResultImage;
    fn new(args: A) -> RvResult<Self>
    where
        Self: Sized;
}

pub trait Cache<A> {
    fn load_from_cache(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage;
    fn new(args: A) -> RvResult<Self>
    where
        Self: Sized;
}
