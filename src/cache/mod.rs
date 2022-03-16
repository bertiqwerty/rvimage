use image::{ImageBuffer, Rgb};

use crate::result::RvResult;

pub mod file_cache;

type ResultImage = RvResult<ImageBuffer<Rgb<u8>, Vec<u8>>>;

type DefaultReader = fn(&str) -> ResultImage;

pub trait ReaderType: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static {}
impl<T: Fn(&str) -> ResultImage + Send + Sync + Clone + 'static> ReaderType for T {}

pub trait Preload<F = DefaultReader>
where
    F: ReaderType,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage;
    fn new(reader: F) -> Self;
}

pub struct NoCache<F = DefaultReader>
where
    F: ReaderType,
{
    reader: F,
}

impl<F> Preload<F> for NoCache<F>
where
    F: ReaderType,
{
    fn read_image(&mut self, selected_file_idx: usize, files: &[String]) -> ResultImage {
        (self.reader)(&files[selected_file_idx])
    }
    fn new(reader: F) -> Self {
        Self { reader }
    }
}
