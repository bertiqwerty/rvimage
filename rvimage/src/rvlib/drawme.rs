use crate::{tools_data::InstanceLabelDisplay, types::ViewImage, world::DataRaw, GeoFig};
use rvimage_domain::{BbF, Canvas, Circle, TPtF};
use std::default::Default;

#[derive(Clone, Debug)]
pub struct Stroke {
    pub thickness: TPtF,
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
pub struct BboxAnnotation {
    pub geofig: GeoFig,
    pub fill_color: Option<[u8; 3]>,
    pub fill_alpha: u8,
    pub outline: Stroke,
    pub outline_alpha: u8,
    pub label: Option<String>,
    pub is_selected: Option<bool>,
    pub highlight_circles: Vec<Circle>,
    pub instance_label_display: InstanceLabelDisplay,
}

#[derive(Clone, Debug)]
pub struct BrushAnnotation {
    pub canvas: Canvas,
    pub color: [u8; 3],
    pub label: Option<String>,
    pub is_selected: Option<bool>,
    pub fill_alpha: u8,
    pub instance_display_label: InstanceLabelDisplay,
}

#[derive(Clone, Debug)]
pub enum Annotation {
    Bbox(BboxAnnotation),
    Brush(BrushAnnotation),
}

#[derive(Clone, Debug, Default)]
pub enum Update<T> {
    Yes(T),
    #[default]
    No,
}

pub type UpdateImage = Update<ViewImage>;
// permament annotations
pub type UpdatePermAnnos = Update<Vec<Annotation>>;
// temporary annotation
pub type UpdateTmpAnno = Update<Annotation>;
pub type UpdateZoomBox = Update<Option<BbF>>;

impl UpdatePermAnnos {
    pub fn clear() -> Self {
        Self::Yes(vec![])
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImageInfo {
    pub filename: String,
    pub shape_info: String,
    pub pixel_value: String,
    pub tool_info: String,
    pub zoom_box_coords: String,
}

#[derive(Clone, Debug, Default)]
pub struct UpdateView {
    pub image: UpdateImage,
    pub perm_annos: UpdatePermAnnos,
    pub tmp_annos: UpdateTmpAnno,
    pub zoom_box: UpdateZoomBox,
    pub image_info: Option<ImageInfo>,

    // to enable memory re-use.
    pub tmp_anno_buffer: Option<Annotation>,
}

impl UpdateView {
    pub fn new(image: &DataRaw, zoom_box: Option<BbF>) -> Self {
        let im_uncropped_view = image.bg_to_uncropped_view();
        UpdateView {
            image: UpdateImage::Yes(im_uncropped_view),
            perm_annos: UpdatePermAnnos::No,
            tmp_annos: UpdateTmpAnno::No,
            zoom_box: UpdateZoomBox::Yes(zoom_box),
            image_info: None,
            tmp_anno_buffer: None,
        }
    }
    pub fn from_zoombox(zoom_box: Option<BbF>) -> Self {
        UpdateView {
            image: UpdateImage::No,
            perm_annos: UpdatePermAnnos::No,
            tmp_annos: UpdateTmpAnno::No,
            zoom_box: UpdateZoomBox::Yes(zoom_box),
            image_info: None,
            tmp_anno_buffer: None,
        }
    }
}
