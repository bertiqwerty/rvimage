use serde::{Deserialize, Serialize};
use std::{
    collections::{btree_map, BTreeMap, HashMap},
    fmt::{Debug, Display},
    mem,
    ops::Index,
    path::Path,
    str::FromStr,
};

use crate::{
    cfg::ExportPath, file_util, implement_annotate, implement_annotations_getters, ShapeI,
};
use rvimage_domain::{rverr, to_rv, RvResult, TPtF, TPtS};

use super::{label_map::LabelMap, ImportExportTrigger};

#[allow(clippy::needless_pass_by_value)]
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
#[serde(untagged)]
pub enum ParamValUntagged {
    Float(Option<TPtF>),
    Int(Option<TPtS>),
    Str(String),
    Bool(bool),
}
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub enum ParamVal {
    Float(Option<TPtF>),
    Int(Option<TPtS>),
    Str(String),
    Bool(bool),
}

impl ParamVal {
    pub fn reset(self) -> Self {
        match self {
            ParamVal::Float(_) => Self::Float(None),
            ParamVal::Int(_) => Self::Int(None),
            ParamVal::Str(_) => Self::Str(String::new()),
            ParamVal::Bool(_) => Self::Bool(false),
        }
    }
    pub fn is_default(&self) -> bool {
        match self {
            ParamVal::Float(x) => x.is_none(),
            ParamVal::Int(x) => x.is_none(),
            ParamVal::Str(x) => x.is_empty(),
            ParamVal::Bool(x) => !x,
        }
    }
    pub fn in_domain_str(&self, domain_str: &str) -> RvResult<bool> {
        let mut min_max_str_it = domain_str.trim().split(ATTR_INTERVAL_SEPARATOR);
        let min_str = min_max_str_it.next().ok_or(rverr!("min not found"))?;
        let max_str = min_max_str_it.next().ok_or(rverr!("max not found"))?;
        macro_rules! unwrap_check {
            ($x:expr) => {
                if let Some(x) = $x {
                    interval_check(*x, min_str, max_str)?
                } else {
                    false
                }
            };
        }
        Ok(match self {
            ParamVal::Float(x) => unwrap_check!(x),
            ParamVal::Int(x) => unwrap_check!(x),
            _ => Err(rverr!(
                "in_domain_str not implemented for the type of {self}"
            ))?,
        })
    }
    #[allow(clippy::float_cmp)]
    pub fn corresponds_to_str(&self, attr_val: &str) -> RvResult<bool> {
        Ok(match self {
            ParamVal::Bool(b) => {
                let attr_val = attr_val.parse::<bool>().map_err(to_rv)?;
                b == &attr_val
            }
            ParamVal::Float(x) => {
                let attr_val = attr_val.parse::<TPtF>().map_err(to_rv)?;
                x == &Some(attr_val)
            }
            ParamVal::Int(x) => {
                let attr_val = attr_val.parse::<TPtS>().map_err(to_rv)?;
                x == &Some(attr_val)
            }
            ParamVal::Str(s) => {
                let attr_val = attr_val.parse::<String>().map_err(to_rv)?;
                s == &attr_val
            }
        })
    }
}

impl Display for ParamVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParamVal::Float(val) => val.map(|val| write!(f, "{val}")).unwrap_or(write!(f, "")),
            ParamVal::Int(val) => val.map(|val| write!(f, "{val}")).unwrap_or(write!(f, "")),
            ParamVal::Str(val) => write!(f, "{val}"),
            ParamVal::Bool(val) => write!(f, "{val}"),
        }
    }
}
impl Default for ParamVal {
    fn default() -> Self {
        ParamVal::Int(None)
    }
}
impl From<ParamValUntagged> for ParamVal {
    fn from(attr_val: ParamValUntagged) -> Self {
        match attr_val {
            ParamValUntagged::Float(x) => ParamVal::Float(x),
            ParamValUntagged::Int(x) => ParamVal::Int(x),
            ParamValUntagged::Str(x) => ParamVal::Str(x),
            ParamValUntagged::Bool(x) => ParamVal::Bool(x),
        }
    }
}

// just for deserialization
pub type ParamMapUntagged = HashMap<String, ParamValUntagged>;

// { attribute name: attribute value }
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct ParamMap {
    #[serde(flatten)]
    data: BTreeMap<String, ParamVal>,
}
impl ParamMap {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ParamVal)> {
        self.data.iter()
    }
    pub fn insert(&mut self, name: String, val: ParamVal) {
        self.data.insert(name, val);
    }
    pub fn get(&self, name: &str) -> Option<&ParamVal> {
        self.data
            .iter()
            .find_map(|(n, v)| if n == name { Some(v) } else { None })
    }
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ParamVal> {
        self.data
            .iter_mut()
            .find_map(|(n, v)| if n == name { Some(v) } else { None })
    }
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.keys()
    }
    pub fn values(&self) -> impl Iterator<Item = &ParamVal> {
        self.data.values()
    }
    pub fn remove(&mut self, name: &str) -> Option<ParamVal> {
        self.data.remove(name)
    }
    pub fn contains(&self, name: &str) -> bool {
        self.data.iter().any(|(n, _)| n == name)
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
impl From<(String, ParamVal)> for ParamMap {
    fn from(data: (String, ParamVal)) -> Self {
        Self {
            data: BTreeMap::from([data]),
        }
    }
}
impl Index<&str> for ParamMap {
    type Output = ParamVal;
    fn index(&self, index: &str) -> &Self::Output {
        self.data
            .iter()
            .find_map(|(n, v)| if n == index { Some(v) } else { None })
            .unwrap_or_else(|| panic!("Attribute {index} not found"))
    }
}
impl IntoIterator for ParamMap {
    type Item = (String, ParamVal);
    type IntoIter = btree_map::IntoIter<String, ParamVal>;
    fn into_iter(self) -> Self::IntoIter {
        self.data.into_iter()
    }
}
impl Index<&String> for ParamMap {
    type Output = ParamVal;
    fn index(&self, index: &String) -> &Self::Output {
        &self[index.as_str()]
    }
}
impl From<HashMap<String, ParamValUntagged>> for ParamMap {
    fn from(data: HashMap<String, ParamValUntagged>) -> Self {
        Self {
            data: data
                .into_iter()
                .map(|(k, v)| (k, ParamVal::from(v)))
                .collect::<BTreeMap<_, _>>(),
        }
    }
}

pub fn merge_attrmaps(mut existing_map: ParamMap, new_map: ParamMap) -> ParamMap {
    for (new_name, new_val) in new_map {
        if let Some(existing_val) = existing_map.get_mut(&new_name) {
            if !new_val.is_default() {
                *existing_val = new_val;
            }
        } else {
            existing_map.insert(new_name, new_val);
        }
    }
    existing_map
}

pub type AttrAnnotationsMap = LabelMap<ParamMap>;

pub fn set_attrmap_val(attr_map: &mut ParamMap, attr_name: &str, attr_val: &ParamVal) {
    attr_map.insert(attr_name.to_string(), attr_val.clone());
}
#[allow(clippy::struct_excessive_bools)]
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct Options {
    pub is_addition_triggered: bool,
    #[serde(default)]
    pub rename_src_idx: Option<usize>, // target idx of renamed attribute
    pub is_update_triggered: bool,
    #[serde(skip)]
    pub import_export_trigger: ImportExportTrigger,
    #[serde(default)]
    pub export_only_opened_folder: bool,
    pub removal_idx: Option<usize>,
}
#[derive(Deserialize, Serialize, Default, Clone, Debug, PartialEq)]
pub struct AttributesToolData {
    attr_names: Vec<String>,
    #[serde(alias = "attr_types")]
    attr_vals: Vec<ParamVal>,
    #[serde(skip)]
    pub new_attr_name: String,
    #[serde(skip)]
    pub new_attr_val: ParamVal,

    // attribute index and value for propagation
    #[serde(skip)]
    pub to_propagate_attr_val: Vec<(usize, ParamVal)>,

    #[serde(alias = "new_attr_buffers")]
    #[serde(alias = "new_attr_name_buffers")]
    new_attr_value_buffers: Vec<String>,
    // maps the filename to the number of rotations
    annotations_map: AttrAnnotationsMap,
    pub options: Options,
    pub current_attr_map: Option<ParamMap>,
    pub export_path: ExportPath,
}
impl AttributesToolData {
    implement_annotations_getters!(ParamMap);
    pub fn rename(&mut self, from_name: &str, to_name: &str) {
        if self.attr_names().iter().any(|n| n == to_name) {
            tracing::warn!("Cannot update to {to_name}. Already exists.");
        } else {
            // better solution: use indices the attr_map hashmap keys instead of Strings
            // rename would then be not necessary anymore.
            let update_attr_map = |attr_map: &mut ParamMap| {
                let keys = attr_map.keys().cloned().collect::<Vec<_>>();
                for k in keys {
                    if k == from_name {
                        let val = attr_map.remove(&k);
                        if let Some(val) = val {
                            attr_map.insert(to_name.to_string(), val);
                        }
                    }
                }
            };
            for (attr_map, _) in self.annotations_map.values_mut() {
                update_attr_map(attr_map);
            }

            if let Some(curmap) = &mut self.current_attr_map {
                update_attr_map(curmap);
            }

            for old_name in &mut self.attr_names {
                if old_name == from_name {
                    *old_name = to_name.to_string();
                }
            }
        }
    }
    pub fn merge_map(&mut self, other: AttrAnnotationsMap) {
        for (filename, (attrmap_other, _)) in other {
            if let Some((attr_map_self, _)) = self.annotations_map.get_mut(&filename) {
                tracing::debug!("Merging {filename} annotations");
                *attr_map_self = merge_attrmaps(mem::take(attr_map_self), attrmap_other);
            } else {
                tracing::debug!("Inserting {filename} annotations");
                self.annotations_map
                    .insert(filename.clone(), (attrmap_other, ShapeI::default()));
            }
        }
    }
    pub fn merge(mut self, other: Self) -> Self {
        self.merge_map(other.annotations_map);
        self
    }
    pub fn push(&mut self, attr_name: String, attr_val: ParamVal) {
        if !self.attr_names.contains(&attr_name) {
            self.attr_names.push(attr_name);
            self.attr_vals.push(attr_val);
            self.new_attr_value_buffers.push(String::new());
        }
    }
    pub fn remove_attr(&mut self, idx: usize) {
        for (_, (attr_map, _)) in self.annotations_map.iter_mut() {
            attr_map.remove(&self.attr_names[idx]);
        }
        self.attr_names.remove(idx);
        self.attr_vals.remove(idx);
        self.new_attr_value_buffers.remove(idx);
    }
    pub fn attr_names(&self) -> &Vec<String> {
        &self.attr_names
    }
    pub fn attr_vals(&self) -> &Vec<ParamVal> {
        &self.attr_vals
    }
    pub fn attr_value_buffer_mut(&mut self, idx: usize) -> &mut String {
        &mut self.new_attr_value_buffers[idx]
    }
    pub fn attr_value_buffers_mut(&mut self) -> &mut Vec<String> {
        &mut self.new_attr_value_buffers
    }
    pub fn set_new_attr_value_buffer(&mut self, buffer: Vec<String>) {
        self.new_attr_value_buffers = buffer;
    }
    pub fn serialize_annotations(&self, key_filter: Option<&str>) -> RvResult<String> {
        if let Some(kf) = key_filter {
            let am = self
                .annotations_map
                .iter()
                .filter_map(|(k, (amap, _))| {
                    if k.contains(kf) {
                        Some((k, amap))
                    } else {
                        None
                    }
                })
                .collect::<HashMap<_, _>>();
            serde_json::to_string(&am).map_err(to_rv)
        } else {
            let am = self
                .annotations_map
                .iter()
                .map(|(k, (amap, _))| (k, amap))
                .collect::<HashMap<_, _>>();
            serde_json::to_string(&am).map_err(to_rv)
        }
    }
    pub fn deserialize_annotations(
        json_str: &str,
        curr_prj_path: Option<&Path>,
    ) -> RvResult<AttrAnnotationsMap> {
        let am: RvResult<HashMap<String, ParamMap>> = serde_json::from_str(json_str).map_err(to_rv);
        let am: RvResult<HashMap<String, ParamMap>> = match am {
            Ok(am) => Ok(am),
            Err(_) => {
                let am: HashMap<String, ParamMapUntagged> =
                    serde_json::from_str(json_str).map_err(to_rv)?;
                Ok(am
                    .into_iter()
                    .map(|(k, v)| (k, ParamMap::from(v)))
                    .collect())
            }
        };
        let am = am.map_err(to_rv)?;
        let mut annotations_map = AttrAnnotationsMap::new();
        for (filename, attr_map) in am.into_iter() {
            let key = file_util::tf_to_annomap_key(filename, curr_prj_path);

            if let Some((self_attr_map, _)) = annotations_map.get_mut(&key) {
                for (attr_name, attr_val) in attr_map.into_iter() {
                    self_attr_map.insert(attr_name, attr_val);
                }
            } else {
                annotations_map.insert(key.clone(), (attr_map, ShapeI::default()));
            }
        }
        tracing::debug!("Annotations map: {annotations_map:?}");
        Ok(annotations_map)
    }
    pub fn attr_map(&self, filename: &str) -> Option<&ParamMap> {
        self.annotations_map
            .get(filename)
            .map(|(attr_map, _)| attr_map)
    }
    pub fn attr_map_mut(&mut self, filename: &str) -> Option<&mut ParamMap> {
        self.annotations_map
            .get_mut(filename)
            .map(|(attr_map, _)| attr_map)
    }
    pub fn get_shape(&self, filename: &str) -> Option<ShapeI> {
        self.annotations_map.get(filename).map(|(_, shape)| *shape)
    }
    pub fn set_attr_val(
        &mut self,
        filename: &str,
        idx: usize,
        attr_val: ParamVal,
        image_shape: ShapeI,
    ) {
        let attr_map = self
            .annotations_map
            .get_mut(filename)
            .map(|(attr_map, _)| attr_map);
        if let Some(attr_map) = attr_map {
            let attr_name = &self.attr_names[idx];
            let current_attr_val = attr_map.get_mut(attr_name);
            if let Some(current_attr_val) = current_attr_val {
                *current_attr_val = attr_val;
            } else {
                attr_map.insert(attr_name.clone(), attr_val);
            }
        } else {
            let attr_map = ParamMap::from((self.attr_names[idx].clone(), attr_val));
            self.annotations_map
                .insert(filename.to_string(), (attr_map, image_shape));
        }
    }
}
implement_annotate!(AttributesToolData);

#[test]
fn test_deserialize() {
    fn test(json_str: &str, expected_len: usize) {
        let newmap = AttributesToolData::deserialize_annotations(json_str, None).unwrap();
        assert_eq!(newmap.len(), expected_len);
    }
    let json_str = r#"{"path": {"attrname": {"Float": 1}}}"#;
    test(json_str, 1);
    let json_str = r#"{"path": {"attrname": {"Float": 1}}, "path1": {"attrname": {"Int": 1}}}"#;
    test(json_str, 2);
}
