use image::{DynamicImage, ImageBuffer, Rgb, imageops::FilterType};

use rvimage_domain::RvResult;

pub type ViewImage = ImageBuffer<Rgb<u8>, Vec<u8>>;
pub type ResultImage = RvResult<DynamicImage>;

pub type AsyncResultImage = RvResult<Option<ImageInfoPair>>;

pub struct ImageInfoPair {
    pub im: DynamicImage,
    pub info: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtraIms {
    pub prev_ims: Vec<DynamicImage>,
    pub next_ims: Vec<DynamicImage>,
}
impl ExtraIms {
    pub fn new(mut prev_ims: Vec<DynamicImage>, mut next_ims: Vec<DynamicImage>) -> Self {
        prev_ims = prev_ims
            .iter()
            .map(|im| im.resize(200, 100, FilterType::Lanczos3))
            .collect();
        next_ims = next_ims
            .iter()
            .map(|im| im.resize(200, 100, FilterType::Lanczos3))
            .collect();
        ExtraIms { prev_ims, next_ims }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtraViews {
    pub prev_ims: Vec<ViewImage>,
    pub next_ims: Vec<ViewImage>,
}
