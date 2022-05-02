use std::marker::PhantomData;

use crate::result::{AsyncResultImage};

use super::{ReadImageToCache, Cache};

pub struct NoCache<F: ReadImageToCache> {
    reader_phantom: PhantomData<F>,
}
impl<F: ReadImageToCache> Cache<()> for NoCache<F> {
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage {
        F::read(&files[selected_file_idx]).map(Some)
    }
    fn new(_: ()) -> Self {
        Self {
            reader_phantom: PhantomData {},
        }
    }
}
