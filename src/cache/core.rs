use crate::result::{ResultImage, AsyncResultImage};

pub trait ReadImageToCache {
    fn read(local_path: &str) -> ResultImage;
}

pub trait Cache<A> {
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage;
    fn new(args: A) -> Self;
}
