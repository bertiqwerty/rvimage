use image::{ImageBuffer, Rgb};

use crate::result::RvResult;


pub type ResultImage = RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>>;

pub trait ImageReaderFn {
    fn read(local_path: &str) -> ResultImage;
}

pub trait Preload<A> {
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage;
    fn new(args: A) -> Self;
}
