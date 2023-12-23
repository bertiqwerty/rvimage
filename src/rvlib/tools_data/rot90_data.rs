use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{implement_annotations_getters, Shape};

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
}

#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct Rot90ToolData {
    // maps the filename to the number of rotations
    annotations_map: HashMap<String, (NRotations, Shape)>,
}
impl Rot90ToolData {
    implement_annotations_getters!(NRotations);
}
