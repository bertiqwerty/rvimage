use std::{
    io,
    path::{Path, PathBuf},
};

use image::{ImageBuffer, Rgb};

type ResultImage = io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>>;

pub trait Preload<F=fn(&Path) -> ResultImage>
where
    F: Fn(&Path) -> ResultImage,
{
    fn preload_images(&mut self, files: &[PathBuf]);
    fn read_image(&self, path: &Path) -> ResultImage;
    fn new(reader: F) -> Self;
}

pub struct NoCache<F=fn(&Path) -> ResultImage>
where
    F: Fn(&Path) -> ResultImage,
{
    reader: F,
}

impl<F> Preload<F> for NoCache<F>
where
    F: Fn(&Path) -> ResultImage,
{
    fn preload_images(&mut self, _: &[PathBuf]) {}
    fn read_image(&self, path: &Path) -> ResultImage {
        (self.reader)(path)
    }
    fn new(reader: F) -> Self {
        Self { reader }
    }
}
