use crate::{
    domain::BrushLine,
    drawme::{Annotation, BboxAnnotation, Stroke},
    result::{trace_ok, RvError, RvResult},
    world::World,
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
        pub fn $func_name(self: $self) -> $crate::result::RvResult<$return_type> {
            match self {
                ToolSpecifics::$variant(x) => Ok(x),
                _ => Err($crate::rverr!("this is not a {}", stringify!($variant))),
            }
        }
    };
}
macro_rules! variant_access_free {
    ($variant:ident, $func_name:ident, $lt:lifetime, $ToolsSpecific:ty, $return_type:ty) => {
        pub fn $func_name<$lt>(x: $ToolsSpecific) -> $crate::result::RvResult<$return_type> {
            match x {
                ToolSpecifics::$variant(x) => Ok(x),
                _ => Err($crate::rverr!("this is not a {}", stringify!($variant))),
            }
        }
    };
}

variant_access_free!(Bbox, bbox, 'a, &'a ToolSpecifics, &'a BboxSpecificData);
variant_access_free!(Bbox, bbox_mut, 'a, &'a mut ToolSpecifics, &'a mut BboxSpecificData);
variant_access_free!(Brush, brush, 'a, &'a ToolSpecifics, &'a BrushToolData);
variant_access_free!(Brush, brush_mut, 'a, &'a mut ToolSpecifics, &'a mut BrushToolData);

pub(super) fn get<'a>(
    world: &'a World,
    actor: &'static str,
    error_msg: &'a str,
) -> RvResult<&'a ToolsData> {
    world
        .data
        .tools_data_map
        .get(actor)
        .ok_or_else(|| RvError::new(error_msg))
}
pub fn get_specific<'a, T>(
    f: impl Fn(&ToolSpecifics) -> RvResult<&T>,
    data: RvResult<&'a ToolsData>,
) -> Option<&'a T> {
    trace_ok(data.and_then(|d| Ok(&d.specifics)).and_then(f))
}
pub(super) fn get_mut<'a>(
    world: &'a mut World,
    actor: &'static str,
    error_msg: &'a str,
) -> RvResult<&'a mut ToolsData> {
    world
        .data
        .tools_data_map
        .get_mut(actor)
        .ok_or_else(|| RvError::new(error_msg))
}
pub fn get_specific_mut<'a, T>(
    f: impl FnMut(&mut ToolSpecifics) -> RvResult<&mut T>,
    data: RvResult<&'a mut ToolsData>,
) -> Option<&'a mut T> {
    trace_ok(data.and_then(|d| Ok(&mut d.specifics)).and_then(f))
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
                    let colors = br_data.label_info.colors();
                    let cats = annos.cat_idxs();
                    let annos = annos
                        .elts()
                        .iter()
                        .zip(cats.iter())
                        .map(
                            |(
                                BrushLine {
                                    line,
                                    intensity,
                                    thickness,
                                },
                                cat_idx,
                            )| {
                                Annotation::Brush(BrushAnnotation {
                                    line: line.clone(),
                                    outline: Stroke {
                                        thickness: *thickness,
                                        color: colors[*cat_idx],
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
