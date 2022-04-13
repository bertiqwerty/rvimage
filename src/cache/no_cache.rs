use std::marker::PhantomData;

use crate::result::{AsyncResultImage};

use super::{ImageReaderFn, Preload};

pub struct NoCache<F: ImageReaderFn> {
    reader_phantom: PhantomData<F>,
}
impl<F: ImageReaderFn> Preload<()> for NoCache<F> {
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage {
        F::read(&files[selected_file_idx]).map(Some)
    }
    fn new(_: ()) -> Self {
        Self {
            reader_phantom: PhantomData {},
        }
    }
}
