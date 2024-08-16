use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    str::FromStr,
};

use crate::{cfg::ExportPath, implement_annotate, implement_annotations_getters, ShapeI};
use rvimage_domain::{rverr, to_rv, RvResult, TPtF, TPtI};

use super::label_map::LabelMap;

fn interval_check<T>(val: T, min: &str, max: &str) -> RvResult<bool>
where
    T: PartialOrd + FromStr + Debug,
    <T as FromStr>::Err: Debug,
{
    let min = min.parse::<T>().map_err(to_rv)?;
    let max = max.parse::<T>().map_err(to_rv)?;
    Ok(val >= min && val <= max)
}

pub const ATTR_INTERVAL_SEPARATOR: &str = "-";

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub enum AttrVal {
    Float(TPtF),
    Int(TPtI),
    Str(String),
    Bool(bool),
}

impl AttrVal {
    pub fn in_domain_str(&self, domain_str: &str) -> RvResult<bool> {
        println!("domain_str: {domain_str}");
        let mut min_max_str_it = domain_str.trim().split(ATTR_INTERVAL_SEPARATOR);
        let min_str = min_max_str_it.next().ok_or(rverr!("min not found"))?;
        let max_str = min_max_str_it.next().ok_or(rverr!("max not found"))?;
        Ok(match self {
            AttrVal::Float(x) => interval_check(*x, min_str, max_str)?,
            AttrVal::Int(x) => interval_check(*x, min_str, max_str)?,
            _ => Err(rverr!(
                "in_domain_str not implemented for the type of {self}"
            ))?,
        })
    }
    pub fn corresponds_to_str(&self, attr_val: &str) -> RvResult<bool> {
        Ok(match self {
            AttrVal::Bool(b) => {
                let attr_val = attr_val.parse::<bool>().map_err(to_rv)?;
                b == &attr_val
            }
            AttrVal::Float(x) => {
                let attr_val = attr_val.parse::<TPtF>().map_err(to_rv)?;
                x == &attr_val
            }
            AttrVal::Int(x) => {
                let attr_val = attr_val.parse::<TPtI>().map_err(to_rv)?;
                x == &attr_val
            }
            AttrVal::Str(s) => {
                let attr_val = attr_val.parse::<String>().map_err(to_rv)?;
                s == &attr_val
            }
        })
    }
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

pub type AttrAnnotationsMap = LabelMap<AttrMap>;

pub fn set_attrmap_val(attr_map: &mut AttrMap, attr_name: &str, attr_val: &AttrVal) {
    attr_map.insert(attr_name.to_string(), attr_val.clone());
}
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct Options {
    pub is_addition_triggered: bool,
    pub is_update_triggered: bool,
    pub is_export_triggered: bool,
    pub removal_idx: Option<usize>,
}
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct AttributesToolData {
    attr_names: Vec<String>,
    #[serde(alias = "attr_types")]
    attr_vals: Vec<AttrVal>,
    #[serde(alias = "new_attr")]
    pub new_attr_name: String,
    #[serde(alias = "new_attr_type")]
    pub new_attr_val: AttrVal,
    #[serde(alias = "new_attr_buffers")]
    new_attr_name_buffers: Vec<String>,
    // maps the filename to the number of rotations
    annotations_map: AttrAnnotationsMap,
    pub options: Options,
    pub current_attr_map: Option<AttrMap>,
    pub export_path: ExportPath,
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
            self.attr_vals.push(attr_val);
            self.new_attr_name_buffers.push(String::new());
        }
    }
    pub fn remove_attr(&mut self, idx: usize) {
        for (_, (attr_map, _)) in self.annotations_map.iter_mut() {
            attr_map.remove(&self.attr_names[idx]);
        }
        self.attr_names.remove(idx);
        self.attr_vals.remove(idx);
        self.new_attr_name_buffers.remove(idx);
    }
    pub fn attr_names(&self) -> &Vec<String> {
        &self.attr_names
    }
    pub fn attr_types(&self) -> &Vec<AttrVal> {
        &self.attr_vals
    }
    pub fn attr_buffer_mut(&mut self, idx: usize) -> &mut String {
        &mut self.new_attr_name_buffers[idx]
    }
    pub fn serialize_annotations(&self) -> RvResult<String> {
        serde_json::to_string(&self.annotations_map).map_err(to_rv)
    }
    pub fn set_annotations_map(&mut self, map: AttrAnnotationsMap) -> RvResult<()> {
        for (_, (attr_map, _)) in map.iter() {
            for attr_name in attr_map.keys() {
                if !self.attr_names.contains(attr_name) {
                    return Err(rverr!("attribute name {attr_name} not found in attr_names"));
                }
            }
        }
        self.annotations_map = map;
        Ok(())
    }
    pub fn attr_map(&mut self, filename: &str) -> Option<&mut AttrMap> {
        self.annotations_map
            .get_mut(filename)
            .map(|(attr_map, _)| attr_map)
    }
}
implement_annotate!(AttributesToolData);
