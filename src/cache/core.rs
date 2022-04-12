use crate::result::{ResultImage, AsyncResultImage};

pub trait ImageReaderFn {
    fn read(local_path: &str) -> ResultImage;
}

pub trait Preload<A> {
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage;
    fn new(args: A) -> Self;
}
