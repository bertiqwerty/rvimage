use image::{DynamicImage, ImageBuffer, Rgb};

use rvimage_domain::RvResult;

pub type ViewImage = ImageBuffer<Rgb<u8>, Vec<u8>>;
pub type ResultImage = RvResult<DynamicImage>;

pub type AsyncResultImage = RvResult<Option<ImageInfoPair>>;

pub struct ImageInfoPair {
    pub im: DynamicImage,
    pub info: String,
}
