use crate::{domain::BrushLine, Line};

use super::core::InstanceAnnotations;

pub type BrushAnnotations = InstanceAnnotations<BrushLine>;

impl BrushAnnotations {
    pub fn last_line_mut(&mut self) -> Option<&mut Line> {
        self.elts_iter_mut().last().map(|x| &mut x.line)
    }
}
