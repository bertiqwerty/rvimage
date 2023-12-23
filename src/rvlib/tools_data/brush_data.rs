use super::annotations::BrushAnnotations;
use crate::domain::Shape;
use crate::implement_annotations_getters;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BrushToolData {
    annotations_map: HashMap<String, (BrushAnnotations, Shape)>,
    pub thickness: f32,
    pub intensity: f32,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
}
impl Default for BrushToolData {
    fn default() -> Self {
        BrushToolData {
            annotations_map: HashMap::new(),
            thickness: 1.0,
            intensity: 1.0,
        }
    }
}
impl Eq for BrushToolData {}
