use crate::domain::Point;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrushAnnotations {
    pub points: Vec<Vec<Point>>,
    pub color: [u8; 3],
}
