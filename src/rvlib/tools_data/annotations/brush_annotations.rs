use crate::Line;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct BrushAnnotations {
    lines: Vec<Line>,
    cat_idxs: Vec<usize>,
    intensities: Vec<f32>,
    thicknesses: Vec<f32>,
}

impl BrushAnnotations {
    pub fn push(&mut self, line: Line, cat_idx: usize, intensity: f32, thickness: f32) {
        self.lines.push(line);
        self.cat_idxs.push(cat_idx);
        self.intensities.push(intensity);
        self.thicknesses.push(thickness);
    }
    pub fn last_line(&mut self) -> Option<&mut Line> {
        self.lines.last_mut()
    }
    pub fn clear(&mut self) {
        self.lines.clear();
        self.cat_idxs.clear();
        self.intensities.clear();
        self.thicknesses.clear();
    }
    pub fn annos_iter(&self) -> impl Iterator<Item = (&Line, usize, f32, f32)> {
        self.lines
            .iter()
            .zip(
                self.cat_idxs
                    .iter()
                    .zip(self.intensities.iter().zip(self.thicknesses.iter())),
            )
            .map(|(line, (c, (i, t)))| (line, *c, *i, *t))
    }
}

impl Eq for BrushAnnotations {}
