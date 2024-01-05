use super::{
    annotations::{BrushAnnotations, ClipboardData},
    core::{self, AnnotationsMap, LabelInfo},
};
use crate::{
    cfg::ExportPath,
    domain::{BrushLine, ShapeI},
};
use crate::{domain::TPtF, implement_annotations_getters};

use serde::{Deserialize, Serialize};

pub type BrushAnnoMap = AnnotationsMap<BrushLine>;

pub const MAX_THICKNESS: f64 = 300.0;
pub const MIN_THICKNESS: f64 = 1.0;
pub const MAX_INTENSITY: f64 = 1.0;
pub const MIN_INTENSITY: f64 = 0.01;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub thickness: TPtF,
    pub intensity: TPtF,
    #[serde(skip)]
    pub erase: bool,
    #[serde(skip)]
    pub is_selection_change_needed: bool,
    #[serde(skip)]
    pub core_options: core::Options,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            thickness: 15.0,
            intensity: 0.5,
            erase: false,
            is_selection_change_needed: false,
            core_options: core::Options::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct BrushToolData {
    pub annotations_map: BrushAnnoMap,
    pub options: Options,
    pub label_info: LabelInfo,
    #[serde(skip)]
    pub clipboard: Option<ClipboardData<BrushLine>>,
    pub export_folder: ExportPath,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
}
impl Eq for BrushToolData {}
