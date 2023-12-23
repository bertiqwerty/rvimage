use super::annotations::BrushAnnotations;
use crate::domain::Shape;
use crate::implement_annotations_getters;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct BrushToolData {
    annotations_map: HashMap<String, (BrushAnnotations, Shape)>,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
}
