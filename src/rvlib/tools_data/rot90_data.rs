use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{implement_annotations_getters, ShapeI};

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
}

#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Rot90ToolData {
    // maps the filename to the number of rotations
    annotations_map: HashMap<String, (NRotations, ShapeI)>,
}
impl Rot90ToolData {
    implement_annotations_getters!(NRotations);
}
