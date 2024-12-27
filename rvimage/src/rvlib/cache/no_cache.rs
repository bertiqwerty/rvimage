use std::marker::PhantomData;

use crate::types::{AsyncResultImage, ImageInfoPair};

use super::{Cache, ReadImageToCache};
use rvimage_domain::RvResult;

pub struct NoCache<RTC, RA>
where
    RTC: ReadImageToCache<RA>,
{
    reader: RTC,
    reader_args_phantom: PhantomData<RA>,
}
impl<RTC: ReadImageToCache<RA>, RA> Cache<RA> for NoCache<RTC, RA> {
    fn load_from_cache(&mut self, selected_file_idx: usize, files: &[&str]) -> AsyncResultImage {
        let path = &files[selected_file_idx];
        self.reader.read(path).map(|im| {
            Some(ImageInfoPair {
                im,
                info: self
                    .reader
                    .file_info(path)
                    .unwrap_or_else(|_| "".to_string()),
            })
        })
    }
    fn ls(&self, folder_path: &str) -> RvResult<Vec<String>> {
        self.reader.ls(folder_path)
    }
    fn new(args: RA) -> RvResult<Self> {
        Ok(Self {
            reader: RTC::new(args)?,
            reader_args_phantom: PhantomData,
        })
    }
    fn size_in_mb(&mut self) -> f64 {
        0.0
    }
    fn clear(&mut self) -> RvResult<()> {
        Ok(())
    }
    fn toggle_clear_on_close(&mut self) {}
}
