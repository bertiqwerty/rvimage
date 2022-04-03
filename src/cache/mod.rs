use std::marker::PhantomData;

use image::{ImageBuffer, Rgb};

use crate::result::RvResult;

pub mod file_cache;

pub type ResultImage = RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>>;

pub trait ImageReaderFn {
    //: Send + Sync + Clone + 'static {
    fn read(local_path: &str) -> ResultImage;
}

pub trait Preload {
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage;
    fn new() -> Self;
}

pub struct NoCache<F: ImageReaderFn> {
    reader_phantom: PhantomData<F>,
}
impl<F: ImageReaderFn> Preload for NoCache<F> {
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage {
        F::read(&files[selected_file_idx])
    }
    fn new() -> Self {
        Self {
            reader_phantom: PhantomData {},
        }
    }
}
