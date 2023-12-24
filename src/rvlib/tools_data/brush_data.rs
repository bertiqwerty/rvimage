use super::{annotations::BrushAnnotations, core::LabelInfo};
use crate::domain::Shape;
use crate::implement_annotations_getters;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub thickness: f32,
    pub intensity: f32,
    pub visible: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            thickness: 1.0,
            intensity: 1.0,
            visible: true
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BrushToolData {
    annotations_map: HashMap<String, (BrushAnnotations, Shape)>,
    pub options: Options,
    pub label_info: LabelInfo
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
}
impl Default for BrushToolData {
    fn default() -> Self {
        BrushToolData {
            annotations_map: HashMap::new(),
            options: Options::default(),
            label_info: LabelInfo::default()
        }
    }
}
impl Eq for BrushToolData {}
