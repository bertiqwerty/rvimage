use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Debug, mem, path::Path};

use crate::{
    ShapeI,
    cfg::ExportPath,
    file_util, implement_annotate, implement_annotations_getters,
    tools_data::parameters::{ParamMapUntagged, merge_attrmaps},
};
use rvimage_domain::{RvResult, to_rv};

use super::{
    ImportExportTrigger,
    label_map::LabelMap,
    parameters::{ParamMap, ParamVal},
};

pub type AttrAnnotationsMap = LabelMap<ParamMap>;

pub fn set_attrmap_val(attr_map: &mut ParamMap, attr_name: &str, attr_val: ParamVal) {
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
    pub annotations_map: AttrAnnotationsMap,
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
            // current map is sorted, hence we need to sort also the lists of attributes
            let mut idxs = (0..self.attr_names.len()).collect::<Vec<_>>();
            idxs.sort_unstable_by_key(|&i| &self.attr_names[i]);
            self.attr_names = idxs.iter().map(|i| self.attr_names[*i].clone()).collect();
            self.attr_vals = idxs.iter().map(|i| self.attr_vals[*i].clone()).collect();
            self.new_attr_value_buffers = idxs
                .iter()
                .map(|i| self.new_attr_value_buffers[*i].clone())
                .collect();
        }
    }
    pub fn remove_attr(&mut self, idx: usize) {
        for (_, (attr_map, _)) in self.annotations_map.iter_mut() {
            attr_map.remove(&self.attr_names[idx]);
        }
        let removed_name = self.attr_names.remove(idx);
        self.attr_vals.remove(idx);
        self.new_attr_value_buffers.remove(idx);
        if let Some(current_attr_map) = &mut self.current_attr_map {
            current_attr_map.remove(&removed_name);
        }
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

#[cfg(test)]
use crate::tracing_setup::init_tracing_for_tests;

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

#[test]
fn test_push_sorted() {
    init_tracing_for_tests();
    let mut data = AttributesToolData::default();
    data.push("c".into(), ParamVal::Int(Some(2)));
    data.push("a".into(), ParamVal::Int(Some(20)));
    assert_eq!(data.attr_names(), &["a", "c"]);
    assert_eq!(
        data.attr_vals(),
        &[ParamVal::Int(Some(20)), ParamVal::Int(Some(2))]
    );
}
