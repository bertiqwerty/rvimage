use super::annotations::BrushAnnotations;
use crate::domain::Shape;
use crate::implement_annotations_getters;
use std::collections::HashMap;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct BrushToolData {
    annotations_map: HashMap<String, (BrushAnnotations, Shape)>,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
}
