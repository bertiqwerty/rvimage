use std::marker::PhantomData;

use super::{core::ResultImage, ImageReaderFn, Preload};

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
