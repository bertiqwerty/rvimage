use crate::{
    domain::{Polygon, BB},
    types::ViewImage,
};
use std::default::Default;

#[derive(Clone, Debug)]
pub struct Stroke {
    pub thickness: f32,
    pub color: [u8; 3],
}

impl Stroke {
    pub fn from_color(color: [u8; 3]) -> Self {
        Stroke {
            thickness: 2.0,
            color,
        }
    }
}

#[derive(Clone, Debug)]
pub enum GeoFig {
    BB(BB),
    Poly(Polygon),
}

#[derive(Clone, Debug)]
pub struct Annotation {
    pub geofig: GeoFig,
    pub fill_color: Option<[u8; 3]>,
    pub fill_alpha: u8,
    pub outline: Stroke,
    pub outline_alpha: u8,
    pub label: Option<String>,
    pub is_selected: Option<bool>,
}

#[derive(Clone, Debug, Default)]
pub enum Update<T> {
    Yes(T),
    #[default]
    No,
}

pub type UpdateImage = Update<ViewImage>;
// permament annotations in the Vec, one temporary annotation in the Option
pub type UpdateAnnos = Update<(Vec<Annotation>, Option<Annotation>)>;
pub type UpdateZoomBox = Update<Option<BB>>;

impl UpdateAnnos {
    pub fn clear() -> Self {
        Self::Yes((vec![], None))
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImageInfo {
    pub filename: String,
    pub shape_info: String,
    pub pixel_value: String,
    pub tool_info: String,
}

#[derive(Clone, Debug, Default)]
pub struct UpdateView {
    pub image: UpdateImage,
    pub annos: UpdateAnnos,
    pub zoom_box: UpdateZoomBox,
    pub image_info: ImageInfo,
}

impl UpdateView {
    pub fn from_zoombox(zoom_box: Option<BB>) -> Self {
        UpdateView {
            image: UpdateImage::No,
            annos: UpdateAnnos::No,
            zoom_box: UpdateZoomBox::Yes(zoom_box),
            image_info: ImageInfo::default(),
        }
    }
}
