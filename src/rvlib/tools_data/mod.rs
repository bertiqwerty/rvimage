use crate::{
    drawme::{Annotation, GeoFig, Stroke},
    UpdateAnnos,
};

pub use self::{
    bbox_data::BboxExportData, bbox_data::BboxSpecificData, brush_data::BrushToolData,
    coco_io::write_coco,
};
pub mod annotations;
pub mod bbox_data;
pub mod brush_data;
pub mod coco_io;

macro_rules! variant_access {
    ($variant:ident, $func_name:ident, $self:ty, $return_type:ty) => {
        pub fn $func_name(self: $self) -> $return_type {
            match self {
                ToolSpecifics::$variant(x) => x,
                _ => panic!("this is not a {}", stringify!($variant)),
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ToolSpecifics {
    Bbox(BboxSpecificData),
    Brush(BrushToolData),
}
impl ToolSpecifics {
    variant_access!(Bbox, bbox, &Self, &BboxSpecificData);
    variant_access!(Brush, brush, &Self, &BrushToolData);
    variant_access!(Bbox, bbox_mut, &mut Self, &mut BboxSpecificData);
    variant_access!(Brush, brush_mut, &mut Self, &mut BrushToolData);

    pub fn to_annotations_view(&self, file_path: &str) -> UpdateAnnos {
        match &self {
            ToolSpecifics::Bbox(bb_data) => {
                if let Some(annos) = bb_data.get_annos(file_path) {
                    let bbs = annos.bbs();
                    let selected_bbs = annos.selected_bbs();
                    let labels = bb_data.labels();
                    let colors = bb_data.colors();
                    let bbs_colored = bbs
                        .iter()
                        .zip(colors.iter().zip(labels.iter().zip(selected_bbs.iter())))
                        .map(|(&bb, (&fill_color, (label, &is_selected)))| Annotation {
                            geofig: GeoFig::BB(bb),
                            fill_color: Some(fill_color),
                            label: Some(label.clone()),
                            outline: Stroke {
                                thickness: 1.0,
                                color: fill_color,
                            },
                            is_selected: Some(is_selected),
                        })
                        .collect::<Vec<Annotation>>();
                    UpdateAnnos::Yes((bbs_colored, None))
                } else {
                    UpdateAnnos::default()
                }
            }
            ToolSpecifics::Brush(brush_data) => {
                // TODO: draw polygon
                UpdateAnnos::default()
            }
        }
    }
}
impl Default for ToolSpecifics {
    fn default() -> Self {
        ToolSpecifics::Bbox(BboxSpecificData::default())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolsData {
    pub specifics: ToolSpecifics,
    pub menu_active: bool,
}
impl ToolsData {
    pub fn new(specifics: ToolSpecifics) -> Self {
        ToolsData {
            specifics,
            menu_active: false,
        }
    }
}
