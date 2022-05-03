use crate::{
    result::{AsyncResultImage, ResultImage, RvResult},
    ImageType,
};

pub trait ReadImageToCache<A> {
    fn read_one(&self, path: &str) -> ResultImage;
    fn read_n(&self, paths: &[&str]) -> RvResult<Vec<ImageType>> {
        paths.iter().map(|p| self.read_one(p)).collect()
    }
    fn new(args: A) -> Self;
}

pub trait Cache<A> {
    fn load_from_cache(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage;
    fn new(args: A) -> Self;
}
