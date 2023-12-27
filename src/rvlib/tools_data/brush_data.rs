use super::{
    annotations::{BrushAnnotations, ClipboardData},
    core::{self, LabelInfo},
};
use crate::domain::{BrushLine, Shape};
use crate::implement_annotations_getters;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub thickness: f32,
    pub intensity: f32,
    pub erase: bool,
    pub core_options: core::Options,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            thickness: 5.0,
            intensity: 1.0,
            erase: false,
            core_options: core::Options::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct BrushToolData {
    pub annotations_map: HashMap<String, (BrushAnnotations, Shape)>,
    pub options: Options,
    pub label_info: LabelInfo,
    pub clipboard: Option<ClipboardData<BrushLine>>,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
}
impl Eq for BrushToolData {}
