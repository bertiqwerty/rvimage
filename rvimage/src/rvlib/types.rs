use image::{DynamicImage, ImageBuffer, Rgb, imageops::FilterType};

use rvimage_domain::RvResult;

use crate::tools_data::parameters::ParamMap;

pub type ViewImage = ImageBuffer<Rgb<u8>, Vec<u8>>;
pub type ResultImage = RvResult<DynamicImage>;

pub type AsyncResultImage = RvResult<Option<ImageInfoPair>>;

pub struct ImageInfoPair {
    pub im: DynamicImage,
    pub info: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageMetaPair {
    pub im: DynamicImage,
    pub meta: ImageMeta,
}
impl ImageMetaPair {
    fn resize(&self, w_max: u32, h_max: u32, filter_type: FilterType) -> Self {
        Self {
            im: self.im.resize(w_max, h_max, filter_type),
            meta: self.meta.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewMetaPair {
    pub im: ViewImage,
    pub meta: ImageMeta,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThumbIms {
    pub prev_ims: Vec<ImageMetaPair>,
    pub im: Option<ImageMetaPair>,
    pub next_ims: Vec<ImageMetaPair>,
}
impl ThumbIms {
    pub fn new(
        prev_ims: Vec<ImageMetaPair>,
        next_ims: Vec<ImageMetaPair>,
        im: Option<&ImageMetaPair>,
        w_max: u32,
        h_max: u32,
    ) -> Self {
        ThumbIms {
            prev_ims: prev_ims
                .iter()
                .map(|im| im.resize(w_max, h_max, FilterType::Lanczos3))
                .collect(),
            im: im.map(|im| im.resize(w_max, h_max, FilterType::Lanczos3)),
            next_ims: next_ims
                .iter()
                .map(|im| im.resize(w_max, h_max, FilterType::Lanczos3))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageMeta {
    pub file_label: String,
    pub attrs: Option<ParamMap>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThumbViews {
    pub prev_ims: Vec<ViewMetaPair>,
    pub im: Option<ViewMetaPair>,
    pub next_ims: Vec<ViewMetaPair>,
}
