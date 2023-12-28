use super::{
    annotations::{BrushAnnotations, ClipboardData},
    core::{self, AnnotationsMap, LabelInfo},
};
use crate::implement_annotations_getters;
use crate::{
    cfg::ExportPath,
    domain::{BrushLine, Shape},
};

use serde::{Deserialize, Serialize};

pub type BrushAnnoMap = AnnotationsMap<BrushLine>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub thickness: f32,
    pub intensity: f32,
    pub erase: bool,
    pub is_selection_change_needed: bool,
    pub core_options: core::Options,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            thickness: 5.0,
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
    pub clipboard: Option<ClipboardData<BrushLine>>,
    pub export_folder: ExportPath,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
}
impl Eq for BrushToolData {}
