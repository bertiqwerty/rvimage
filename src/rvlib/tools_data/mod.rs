use crate::{
    domain::BrushLine,
    drawme::{Annotation, BboxAnnotation, Stroke},
    BrushAnnotation, UpdateAnnos,
};

pub use self::core::{LabelInfo, OUTLINE_THICKNESS_CONVERSION};
pub use self::{
    bbox_data::BboxExportData, bbox_data::BboxSpecificData, brush_data::BrushToolData,
    coco_io::write_coco, rot90_data::Rot90ToolData,
};
use serde::{Deserialize, Serialize};
pub mod annotations;
pub mod bbox_data;
pub mod brush_data;
pub mod coco_io;
mod core;
pub mod rot90_data;

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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ToolSpecifics {
    Bbox(BboxSpecificData),
    Brush(BrushToolData),
    Rot90(Rot90ToolData),
}
impl ToolSpecifics {
    variant_access!(Bbox, bbox, &Self, &BboxSpecificData);
    variant_access!(Brush, brush, &Self, &BrushToolData);
    variant_access!(Rot90, rot90, &Self, &Rot90ToolData);
    variant_access!(Bbox, bbox_mut, &mut Self, &mut BboxSpecificData);
    variant_access!(Brush, brush_mut, &mut Self, &mut BrushToolData);
    variant_access!(Rot90, rot90_mut, &mut Self, &mut Rot90ToolData);

    pub fn to_annotations_view(&self, file_path: &str) -> UpdateAnnos {
        match &self {
            ToolSpecifics::Bbox(bb_data) => {
                if let Some(annos) = bb_data.get_annos(file_path) {
                    let bbs = annos.elts();
                    let cats = annos.cat_idxs();
                    let selected_bbs = annos.selected_mask();
                    let labels = bb_data.label_info.labels();
                    let colors = bb_data.label_info.colors();

                    let bbs_colored = bbs
                        .iter()
                        .zip(cats.iter())
                        .zip(selected_bbs.iter())
                        .map(|((bb, cat_idx), is_selected)| {
                            Annotation::Bbox(BboxAnnotation {
                                geofig: bb.clone(),
                                fill_color: Some(colors[*cat_idx]),
                                fill_alpha: bb_data.options.fill_alpha,
                                label: Some(labels[*cat_idx].clone()),
                                outline: Stroke {
                                    thickness: bb_data.options.outline_thickness as f32
                                        / OUTLINE_THICKNESS_CONVERSION,
                                    color: colors[*cat_idx],
                                },
                                outline_alpha: bb_data.options.outline_alpha,
                                is_selected: Some(*is_selected),
                            })
                        })
                        .collect::<Vec<Annotation>>();
                    UpdateAnnos::Yes((bbs_colored, None))
                } else {
                    UpdateAnnos::clear()
                }
            }
            ToolSpecifics::Brush(br_data) => {
                if let Some(annos) = br_data.get_annos(file_path) {
                    let annos = annos
                        .elts()
                        .iter()
                        .map(
                            |BrushLine {
                                 line,
                                 intensity,
                                 thickness,
                             }| {
                                Annotation::Brush(BrushAnnotation {
                                    line: line.clone(),
                                    outline: Stroke {
                                        thickness: *thickness,
                                        color: [255, 255, 255],
                                    },
                                    intensity: *intensity,
                                    label: None,
                                })
                            },
                        )
                        .collect::<Vec<Annotation>>();
                    UpdateAnnos::Yes((annos, None))
                } else {
                    UpdateAnnos::clear()
                }
            }
            ToolSpecifics::Rot90(_) => UpdateAnnos::default(),
        }
    }
}
impl Default for ToolSpecifics {
    fn default() -> Self {
        ToolSpecifics::Bbox(BboxSpecificData::default())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
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
