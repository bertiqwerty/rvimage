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
pub struct ExtraIms {
    pub prev_ims: Vec<DynamicImage>,
    pub next_ims: Vec<DynamicImage>,
    pub prev_meta: Vec<ExtraMeta>,
    pub next_meta: Vec<ExtraMeta>,
}
impl ExtraIms {
    pub fn new(
        mut prev_ims: Vec<DynamicImage>,
        mut next_ims: Vec<DynamicImage>,
        w_max: u32,
        h_max: u32,
        prev_meta: Vec<ExtraMeta>,
        next_meta: Vec<ExtraMeta>,
    ) -> Self {
        prev_ims = prev_ims
            .iter()
            .map(|im| im.resize(w_max, h_max, FilterType::Lanczos3))
            .collect();
        next_ims = next_ims
            .iter()
            .map(|im| im.resize(w_max, h_max, FilterType::Lanczos3))
            .collect();
        ExtraIms {
            prev_ims,
            next_ims,
            prev_meta,
            next_meta,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtraMeta {
    pub abs_file_path: String,
    pub attrs: Option<ParamMap>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtraViews {
    pub prev_ims: Vec<ViewImage>,
    pub next_ims: Vec<ViewImage>,
    pub prev_meta: Vec<ExtraMeta>,
    pub next_meta: Vec<ExtraMeta>,
}
