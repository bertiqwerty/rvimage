use crate::{
    drawme::{Annotation, BboxAnnotation, Stroke},
    BrushAnnotation,
};

pub use self::core::{
    vis_from_lfoption, Annotate, ExportAsCoco, ImportExportTrigger, ImportMode, InstanceAnnotate,
    InstanceExportData, LabelInfo, Options, VisibleInactiveToolsState,
    OUTLINE_THICKNESS_CONVERSION,
};
pub use self::{
    attributes_data::AttributesToolData, bbox_data::BboxToolData, brush_data::BrushToolData,
    coco_io::write_coco, rot90_data::Rot90ToolData,
};
use rvimage_domain::{rverr, RvResult, TPtF};
use serde::{Deserialize, Serialize};
pub mod annotations;
pub mod attributes_data;
pub mod bbox_data;
pub mod brush_data;
pub mod coco_io;
mod core;
mod label_map;
pub mod rot90_data;
pub use core::{merge, AnnotationsMap, Options as CoreOptions};

macro_rules! variant_access {
    ($variant:ident, $func_name:ident, $self:ty, $return_type:ty) => {
        pub fn $func_name(self: $self) -> rvimage_domain::RvResult<$return_type> {
            match self {
                ToolSpecifics::$variant(x) => Ok(x),
                _ => Err(rvimage_domain::rverr!(
                    "this is not a {}",
                    stringify!($variant)
                )),
            }
        }
    };
}
macro_rules! variant_access_free {
    ($variant:ident, $func_name:ident, $lt:lifetime, $ToolsSpecific:ty, $return_type:ty) => {
        pub fn $func_name<$lt>(x: $ToolsSpecific) -> rvimage_domain::RvResult<$return_type> {
            match x {
                ToolSpecifics::$variant(x) => Ok(x),
                _ => Err(rvimage_domain::rverr!(
                    "this is not a {}",
                    stringify!($variant)
                )),
            }
        }
    };
}

variant_access_free!(Bbox, bbox, 'a, &'a ToolSpecifics, &'a BboxToolData);
variant_access_free!(Bbox, bbox_mut, 'a, &'a mut ToolSpecifics, &'a mut BboxToolData);
variant_access_free!(Brush, brush, 'a, &'a ToolSpecifics, &'a BrushToolData);
variant_access_free!(Brush, brush_mut, 'a, &'a mut ToolSpecifics, &'a mut BrushToolData);
variant_access_free!(Attributes, attributes, 'a, &'a ToolSpecifics, &'a AttributesToolData);
variant_access_free!(Attributes, attributes_mut, 'a, &'a mut ToolSpecifics, &'a mut AttributesToolData);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ToolSpecifics {
    Bbox(BboxToolData),
    Brush(BrushToolData),
    Rot90(Rot90ToolData),
    Zoom(()),
    AlwaysActiveZoom(()),
    Attributes(AttributesToolData),
}
impl ToolSpecifics {
    variant_access!(Bbox, bbox, &Self, &BboxToolData);
    variant_access!(Brush, brush, &Self, &BrushToolData);
    variant_access!(Rot90, rot90, &Self, &Rot90ToolData);
    variant_access!(Attributes, attributes, &Self, &AttributesToolData);
    variant_access!(Bbox, bbox_mut, &mut Self, &mut BboxToolData);
    variant_access!(Brush, brush_mut, &mut Self, &mut BrushToolData);
    variant_access!(Rot90, rot90_mut, &mut Self, &mut Rot90ToolData);
    variant_access!(
        Attributes,
        attributes_mut,
        &mut Self,
        &mut AttributesToolData
    );

    pub fn apply_mut<T>(
        &mut self,
        mut f_bbox: impl FnMut(&mut BboxToolData) -> RvResult<T>,
        mut f_brush: impl FnMut(&mut BrushToolData) -> RvResult<T>,
    ) -> RvResult<T> {
        match self {
            Self::Bbox(bbox_data) => f_bbox(bbox_data),
            Self::Brush(brush_data) => f_brush(brush_data),
            _ => Err(rverr!("only brush tool and bbox tool can be used in apply")),
        }
    }
    pub fn apply<T>(
        &self,
        mut f_bbox: impl FnMut(&BboxToolData) -> RvResult<T>,
        mut f_brush: impl FnMut(&BrushToolData) -> RvResult<T>,
    ) -> RvResult<T> {
        match self {
            Self::Bbox(bbox_data) => f_bbox(bbox_data),
            Self::Brush(brush_data) => f_brush(brush_data),
            _ => Err(rverr!("only brush tool and bbox tool can be used in apply")),
        }
    }

    pub fn to_annotations_view(
        &self,
        file_path_relative: &str,
        only_cat_idx: Option<usize>,
    ) -> Option<Vec<Annotation>> {
        match self {
            ToolSpecifics::Bbox(bb_data) => {
                if let Some(annos) = bb_data.get_annos(file_path_relative) {
                    let geos = annos.elts();
                    let cats = annos.cat_idxs();
                    let selected_bbs = annos.selected_mask();
                    let labels = bb_data.label_info.labels();
                    let colors = bb_data.label_info.colors();
                    let bbs_colored = geos
                        .iter()
                        .zip(cats.iter())
                        .zip(selected_bbs.iter())
                        .filter(|((_, cat_idx), _)| {
                            if let Some(only_cat_idx) = only_cat_idx {
                                **cat_idx == only_cat_idx
                            } else {
                                true
                            }
                        })
                        .map(|((geo, cat_idx), is_selected)| {
                            Annotation::Bbox(BboxAnnotation {
                                geofig: geo.clone(),
                                fill_color: Some(colors[*cat_idx]),
                                fill_alpha: bb_data.options.fill_alpha,
                                label: Some(labels[*cat_idx].clone()),
                                outline: Stroke {
                                    thickness: bb_data.options.outline_thickness as TPtF
                                        / OUTLINE_THICKNESS_CONVERSION,
                                    color: colors[*cat_idx],
                                },
                                outline_alpha: bb_data.options.outline_alpha,
                                is_selected: Some(*is_selected),
                                highlight_circles: bb_data.highlight_circles.clone(),
                            })
                        })
                        .collect::<Vec<Annotation>>();
                    Some(bbs_colored)
                } else {
                    Some(vec![])
                }
            }
            ToolSpecifics::Brush(br_data) => {
                if let Some(annos) = br_data.get_annos(file_path_relative) {
                    let colors = br_data.label_info.colors();
                    let cats = annos.cat_idxs();
                    let selected_mask = annos.selected_mask();
                    let annos = annos
                        .elts()
                        .iter()
                        .zip(cats.iter())
                        .zip(selected_mask.iter())
                        .filter(|((_, cat_idx), _)| {
                            if let Some(only_cat_idx) = only_cat_idx {
                                **cat_idx == only_cat_idx
                            } else {
                                true
                            }
                        })
                        .map(|((brush_line, cat_idx), is_selected)| {
                            let tmp_line = if let Some((tmp_line, tmp_cat_idx)) = &br_data.tmp_line
                            {
                                if tmp_cat_idx == cat_idx {
                                    Some(tmp_line)
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            Annotation::Brush(BrushAnnotation {
                                canvas: brush_line.clone(),
                                tmp_line: tmp_line.cloned(),
                                color: colors[*cat_idx],
                                label: None,
                                is_selected: Some(*is_selected),
                                fill_alpha: br_data.options.fill_alpha,
                            })
                        })
                        .collect::<Vec<Annotation>>();
                    Some(annos)
                } else {
                    Some(vec![])
                }
            }
            _ => None,
        }
    }
}
impl Default for ToolSpecifics {
    fn default() -> Self {
        ToolSpecifics::Bbox(BboxToolData::default())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ToolsData {
    pub specifics: ToolSpecifics,
    pub menu_active: bool,
    #[serde(default)]
    pub visible_inactive_tools: VisibleInactiveToolsState,
}
impl ToolsData {
    pub fn new(
        specifics: ToolSpecifics,
        visible_inactive_tools: VisibleInactiveToolsState,
    ) -> Self {
        ToolsData {
            specifics,
            menu_active: false,
            visible_inactive_tools,
        }
    }
}
