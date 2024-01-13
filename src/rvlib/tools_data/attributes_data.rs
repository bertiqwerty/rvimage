use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    domain::{TPtF, TPtI},
    implement_annotations_getters, ShapeI,
};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub enum AttrVal {
    Float(TPtF),
    Int(TPtI),
    Str(String),
    Bool(bool),
}
impl Default for AttrVal {
    fn default() -> Self {
        AttrVal::Int(0)
    }
}

pub type AttrMap = HashMap<String, AttrVal>;

#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct AttributesToolData {
    // maps the filename to the number of rotations
    annotations_map: HashMap<String, (AttrMap, ShapeI)>,
}
impl AttributesToolData {
    implement_annotations_getters!(AttrMap);
    pub fn merge(mut self, other: Self) -> Self {
        for (filename, (attrmap_other, _)) in other.annotations_map {
            if let Some((attr_map_self, _)) = self.annotations_map.get_mut(&filename) {
                for (attr_name, attr_val) in attrmap_other {
                    if !attr_map_self.contains_key(&attr_name) {
                        attr_map_self.insert(attr_name, attr_val);
                    }
                }
            }
        }
        self
    }
}
