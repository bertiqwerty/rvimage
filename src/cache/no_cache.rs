use std::marker::PhantomData;

use crate::result::AsyncResultImage;

use super::{Cache, ReadImageToCache};

pub struct NoCache<RTC, RA>
where
    RTC: ReadImageToCache<RA>,
{
    reader: RTC,
    reader_args_phantom: PhantomData<RA>,
}
impl<RTC: ReadImageToCache<RA>, RA> Cache<RA> for NoCache<RTC, RA> {
    fn load_from_cache(&mut self, selected_file_idx: usize, files: &[String]) -> AsyncResultImage {
        self.reader.read_one(&files[selected_file_idx]).map(Some)
    }
    fn new(args: RA) -> Self {
        Self {
            reader: RTC::new(args),
            reader_args_phantom: PhantomData,
        }
    }
}
