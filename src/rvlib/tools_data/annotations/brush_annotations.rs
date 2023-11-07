use crate::domain::PtI;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrushAnnotations {
    pub points: Vec<Vec<PtI>>,
    pub color: [u8; 3],
}
