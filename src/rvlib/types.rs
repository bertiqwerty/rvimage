use image::{ImageBuffer, Rgb, DynamicImage};

use crate::result::RvResult;


pub type ViewImage = ImageBuffer<Rgb<u8>, Vec<u8>>;
pub type ResultImage = RvResult<DynamicImage>;

pub type AsyncResultImage = RvResult<Option<DynamicImage>>;