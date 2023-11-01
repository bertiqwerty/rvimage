use crate::{
    domain::{Point, Polygon, BB},
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
            thickness: 1.0,
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
    pub fill_color: [u8; 3],
    pub outline: Stroke,
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
pub type UpdateAnnos = Update<Vec<Annotation>>;
pub type UpdateZoomBox = Update<Option<BB>>;

#[derive(Clone, Debug, Default)]
pub struct UpdateView {
    pub image: UpdateImage,
    pub annos: UpdateAnnos,
    pub zoom_box: UpdateZoomBox,
    pub image_info: String,
}

impl UpdateView {
    pub fn from_zoombox(zoom_box: Option<BB>) -> Self {
        UpdateView {
            image: UpdateImage::No,
            annos: UpdateAnnos::No,
            zoom_box: UpdateZoomBox::Yes(zoom_box),
            image_info: "".to_string(),
        }
    }
}
