use crate::domain::PtI;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BrushAnnotations {
    pub points: Vec<Vec<PtI>>,
    pub color: [u8; 3],
}
