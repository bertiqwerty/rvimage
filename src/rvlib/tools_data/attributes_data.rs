use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use crate::{
    domain::{TPtF, TPtI},
    implement_annotations_getters, ShapeI,
};

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub enum AttrVal {
    Float(TPtF),
    Int(TPtI),
    Str(String),
    Bool(bool),
}

impl Display for AttrVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttrVal::Float(val) => write!(f, "{}", val),
            AttrVal::Int(val) => write!(f, "{}", val),
            AttrVal::Str(val) => write!(f, "{}", val),
            AttrVal::Bool(val) => write!(f, "{}", val),
        }
    }
}
impl Default for AttrVal {
    fn default() -> Self {
        AttrVal::Int(0)
    }
}

// { attribute name: attribute value }
pub type AttrMap = HashMap<String, AttrVal>;

pub fn set_attrmap_val(attr_map: &mut AttrMap, attr_name: &str, attr_val: &AttrVal) {
    attr_map.insert(attr_name.to_string(), attr_val.clone());
}
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct Options {
    pub populate_new_attr: bool,
    pub update_current_attr_map: bool,
}
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct AttributesToolData {
    attr_names: Vec<String>,
    attr_types: Vec<AttrVal>,
    pub new_attr: String,
    pub new_attr_type: AttrVal,
    new_attr_buffers: Vec<String>,
    // maps the filename to the number of rotations
    pub annotations_map: HashMap<String, (AttrMap, ShapeI)>,
    pub options: Options,
    pub current_attr_map: Option<AttrMap>,
}
impl AttributesToolData {
    implement_annotations_getters!(AttrMap);
    pub fn merge(mut self, other: Self) -> Self {
        for (filename, (attrmap_other, _)) in other.annotations_map {
            if let Some((attr_map_self, _)) = self.annotations_map.get_mut(&filename) {
                for (attr_name, attr_val) in attrmap_other {
                    attr_map_self.entry(attr_name).or_insert(attr_val);
                }
            }
        }
        self
    }
    pub fn push(&mut self, attr_name: String, attr_val: AttrVal) {
        if !self.attr_names.contains(&attr_name) {
            self.attr_names.push(attr_name);
            self.attr_types.push(attr_val);
            self.new_attr_buffers.push(String::new());
        }
    }
    pub fn remove_attr(&mut self, idx: usize) {
        self.attr_names.remove(idx);
        self.attr_types.remove(idx);
        self.new_attr_buffers.remove(idx);
    }
    pub fn attr_names(&self) -> &Vec<String> {
        &self.attr_names
    }
    pub fn attr_types(&self) -> &Vec<AttrVal> {
        &self.attr_types
    }
    pub fn attr_buffer_mut(&mut self, idx: usize) -> &mut String {
        &mut self.new_attr_buffers[idx]
    }
}
