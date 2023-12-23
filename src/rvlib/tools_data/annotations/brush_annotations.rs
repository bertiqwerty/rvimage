use crate::Line;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BrushAnnotations {
    pub lines: Vec<Line>,
    pub cat_idx: Vec<usize>,
}
