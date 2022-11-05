use std::collections::HashMap;

use crate::{annotations::BrushAnnotations, implement_annotations_getters};

const fn default() -> BrushAnnotations {
    BrushAnnotations {
        points: vec![],
        color: [255, 255, 255],
    }
}

static DEFAULT_BRUSH_ANNOTATION: BrushAnnotations = default();
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct BrushSpecifics {
    annotations_map: HashMap<String, BrushAnnotations>,
}
impl BrushSpecifics {
    implement_annotations_getters!(&DEFAULT_BRUSH_ANNOTATION, BrushAnnotations);
}
