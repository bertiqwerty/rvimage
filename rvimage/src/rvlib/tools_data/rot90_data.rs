use serde::{Deserialize, Serialize};

use crate::{implement_annotate, implement_annotations_getters, ShapeI};

use super::label_map::LabelMap;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Default, Copy)]
pub enum NRotations {
    #[default]
    Zero,
    One,
    Two,
    Three,
}

impl NRotations {
    pub fn increase(self) -> Self {
        match self {
            Self::Zero => Self::One,
            Self::One => Self::Two,
            Self::Two => Self::Three,
            Self::Three => Self::Zero,
        }
    }
    pub fn to_num(self) -> u8 {
        match self {
            Self::Zero => 0,
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
    pub fn max(self, other: Self) -> Self {
        if self.to_num() >= other.to_num() {
            self
        } else {
            other
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Zero)
    }
}

pub type Rot90AnnotationsMap = LabelMap<NRotations>;

#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Rot90ToolData {
    // maps the filename to the number of rotations
    annotations_map: Rot90AnnotationsMap,
}
impl Rot90ToolData {
    implement_annotations_getters!(NRotations);
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        for (filename, (nrot_other, shape)) in other.annotations_map {
            let nrot = if let Some((nrot_self, _)) = self.annotations_map.get(&filename) {
                nrot_self.max(nrot_other)
            } else {
                nrot_other
            };
            self.annotations_map.insert(filename, (nrot, shape));
        }
        self
    }
    pub fn set_annotations_map(&mut self, map: Rot90AnnotationsMap) {
        self.annotations_map = map;
    }
}

implement_annotate!(Rot90ToolData);
