use std::{
    io,
    path::{Path, PathBuf},
};

use image::{ImageBuffer, Rgb};

type ResultImage = io::Result<ImageBuffer<Rgb<u8>, Vec<u8>>>;

pub trait Preload {
    fn preload_images(&mut self, files: &[PathBuf]);
    fn read_image(&self, path: &Path) -> ResultImage;
    fn new(reader: fn(&Path) -> ResultImage) -> Self;
}

pub struct NoCache {
    reader: fn(&Path) -> ResultImage,
}

impl Preload for NoCache {
    fn preload_images(&mut self, _: &[PathBuf]) {}
    fn read_image(&self, path: &Path) -> ResultImage {
        (self.reader)(path)
    }
    fn new(reader: fn(&Path) -> ResultImage) -> Self {
        Self { reader }
    }
}
