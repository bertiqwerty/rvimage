pub use self::core::{
    vis_from_lfoption, AccessInstanceData, Annotate, ExportAsCoco, ImportExportTrigger, ImportMode,
    InstanceAnnotate, InstanceExportData, LabelInfo, Options, VisibleInactiveToolsState,
    OUTLINE_THICKNESS_CONVERSION,
};
pub use self::{
    attributes_data::AttributesToolData, bbox_data::BboxToolData, brush_data::BrushToolData,
    coco_io::write_coco, plot_stats::PlotAnnotationStats, rot90_data::Rot90ToolData,
};
use crate::tools::add_tools_initial_data;
use crate::{
    drawme::{Annotation, BboxAnnotation, Stroke},
    BrushAnnotation,
};
use rvimage_domain::{rverr, RvResult, TPtF};
use serde::{Deserialize, Serialize};
use std::ops::Index;

pub mod annotations;
pub mod attributes_data;
pub mod bbox_data;
pub mod brush_data;
pub mod coco_io;
mod core;
mod label_map;
pub mod parameters;
mod plot_stats;
pub mod predictive_labeling;
pub mod rot90_data;
pub use core::{merge, AnnotationsMap, InstanceLabelDisplay, Options as CoreOptions};
use std::collections::HashMap;

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
        f_bbox: impl FnOnce(&mut BboxToolData) -> RvResult<T>,
        f_brush: impl FnOnce(&mut BrushToolData) -> RvResult<T>,
        f_attr: impl FnOnce(&mut AttributesToolData) -> RvResult<T>,
    ) -> RvResult<T> {
        match self {
            Self::Bbox(bbox_data) => f_bbox(bbox_data),
            Self::Brush(brush_data) => f_brush(brush_data),
            Self::Attributes(attr_data) => f_attr(attr_data),
            _ => Err(rverr!("only brush tool and bbox tool can be used in apply")),
        }
    }
    pub fn apply<T>(
        &self,
        f_bbox: impl FnOnce(&BboxToolData) -> RvResult<T>,
        f_brush: impl FnOnce(&BrushToolData) -> RvResult<T>,
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
                                    thickness: TPtF::from(bb_data.options.outline_thickness)
                                        / OUTLINE_THICKNESS_CONVERSION,
                                    color: colors[*cat_idx],
                                },
                                outline_alpha: bb_data.options.outline_alpha,
                                is_selected: Some(*is_selected),
                                highlight_circles: bb_data.highlight_circles.clone(),
                                instance_label_display: bb_data.options.core.instance_label_display,
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
                    let labels = br_data.label_info.labels();
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
                            Annotation::Brush(BrushAnnotation {
                                canvas: brush_line.clone(),
                                color: colors[*cat_idx],
                                label: Some(labels[*cat_idx].clone()),
                                is_selected: Some(*is_selected),
                                fill_alpha: br_data.options.fill_alpha,
                                instance_display_label: br_data.options.core.instance_label_display,
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

#[macro_export]
macro_rules! toolsdata_by_name {
    ($name:expr, $acc:ident, $tdm:expr) => {
        $tdm.get_mut($name)
            .ok_or(rvimage_domain::rverr!("{} is not a tool", $name))?
            .specifics
            .$acc()?
    };
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolsDataMap {
    // tool name -> tool's menu data type
    #[serde(flatten)]
    data: HashMap<String, ToolsData>,
}
impl ToolsDataMap {
    pub fn new() -> Self {
        let tdm = ToolsDataMap {
            data: HashMap::new(),
        };
        add_tools_initial_data(tdm)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ToolsData)> {
        self.data.iter()
    }
    pub fn contains_key(&self, name: &str) -> bool {
        self.data.contains_key(name)
    }
    pub fn get_specifics(&self, name: &str) -> Option<&ToolSpecifics> {
        self.data.get(name).map(|d| &d.specifics)
    }
    pub fn get_specifics_mut(&mut self, name: &str) -> Option<&mut ToolSpecifics> {
        self.data.get_mut(name).map(|d| &mut d.specifics)
    }
    pub fn get(&self, name: &str) -> Option<&ToolsData> {
        self.data.get(name)
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ToolsData> {
        self.data.get_mut(name)
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut ToolsData> {
        self.data.values_mut()
    }

    pub fn insert(&mut self, name: String, data: ToolsData) -> Option<ToolsData> {
        self.data.insert(name, data)
    }
    pub fn set_tools_specific_data(&mut self, name: &str, specifics: ToolSpecifics) {
        self.data.insert(
            name.to_string(),
            ToolsData::new(specifics, VisibleInactiveToolsState::default()),
        );
    }
}
impl Default for ToolsDataMap {
    fn default() -> Self {
        Self::new()
    }
}
impl Index<&str> for ToolsDataMap {
    type Output = ToolsData;
    fn index(&self, index: &str) -> &Self::Output {
        &self.data[index]
    }
}
impl FromIterator<(String, ToolsData)> for ToolsDataMap {
    fn from_iter<T: IntoIterator<Item = (std::string::String, ToolsData)>>(iter: T) -> Self {
        let data = iter.into_iter().collect::<HashMap<String, ToolsData>>();
        add_tools_initial_data(ToolsDataMap { data })
    }
}
impl From<HashMap<String, ToolsData>> for ToolsDataMap {
    fn from(data: HashMap<String, ToolsData>) -> Self {
        add_tools_initial_data(ToolsDataMap { data })
    }
}

#[macro_export]
macro_rules! get_specifics_from_tdm {
    ($actor_name:expr, $tdm:expr, $access_func:ident) => {
        $tdm.get($actor_name)
            .and_then(|x| x.specifics.$access_func().ok())
    };
}
#[macro_export]
macro_rules! get_annos_from_tdm {
    ($actor_name:expr, $tdm:expr, $current_file_path:expr, $access_func:ident) => {
        $crate::get_specifics_from_tdm!($actor_name, $tdm, $access_func)
            .and_then(|d| d.get_annos($current_file_path))
    };
}

#[macro_export]
macro_rules! get_labelinfo_from_tdm {
    ($actor_name:expr, $tdm:expr,  $access_func:ident) => {
        $crate::get_specifics_from_tdm!($actor_name, $tdm, $access_func).map(|d| d.label_info())
    };
}

#[cfg(test)]
use crate::tools::{
    ALWAYS_ACTIVE_ZOOM, ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME, ROT90_NAME, ZOOM_NAME,
};
#[test]
fn test_tools_data_map() {
    let tdm = ToolsDataMap::new();
    let tools = [
        BBOX_NAME,
        ROT90_NAME,
        BRUSH_NAME,
        ATTRIBUTES_NAME,
        ZOOM_NAME,
        ALWAYS_ACTIVE_ZOOM,
    ];
    for tool in tools.iter() {
        assert!(tdm.contains_key(tool));
    }
    assert_eq!(tdm.len(), tools.len());

    // test from hashmap
    let data = HashMap::from([(BBOX_NAME.to_string(), ToolsData::default())]);
    let tdm = ToolsDataMap::from(data);
    for tool in tools.iter() {
        assert!(tdm.contains_key(tool));
    }
    assert_eq!(tdm.len(), tools.len());

    // test from iterator
    let data = vec![
        (BBOX_NAME.to_string(), ToolsData::default()),
        (ROT90_NAME.to_string(), ToolsData::default()),
    ];
    let tdm = ToolsDataMap::from_iter(data);
    for tool in tools.iter() {
        assert!(tdm.contains_key(tool));
    }
    assert_eq!(tdm.len(), tools.len());
}
