use std::{collections::HashMap, ops::Index, path::Path};

use rvimage_domain::{rverr, to_rv, ShapeI};
use serde::{de::DeserializeOwned, Deserialize, Serialize, Serializer};

use crate::{cfg::read_cfg, file_util::path_to_str, result::trace_ok_err};

pub fn tf_to_annomap_key(path: String, curr_prj_path: Option<&Path>) -> String {
    if let Some(curr_prj_path) = curr_prj_path {
        let path_ref = Path::new(&path);
        let prj_parent = curr_prj_path
            .parent()
            .ok_or_else(|| rverr!("{curr_prj_path:?} has no parent"));
        let relative_path =
            prj_parent.and_then(|prj_parent| path_ref.strip_prefix(prj_parent).map_err(to_rv));
        if let Ok(relative_path) = relative_path {
            let without_base = path_to_str(relative_path);
            if let Ok(without_base) = without_base {
                without_base.to_string()
            } else {
                path
            }
        } else {
            path
        }
    } else {
        path
    }
}

fn serialize_relative_paths<S, T>(
    data: &HashMap<String, (T, ShapeI)>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    let cfg = trace_ok_err(read_cfg());

    data.iter()
        .map(|(k, (v, s))| {
            (
                tf_to_annomap_key(k.clone(), cfg.as_ref().map(|cfg| cfg.current_prj_path())),
                (v, *s),
            )
        })
        .collect::<HashMap<_, _>>()
        .serialize(serializer)
}

fn deserialize_relative_paths<'de, D, T>(
    deserializer: D,
) -> Result<HashMap<String, (T, ShapeI)>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned,
{
    let cfg = trace_ok_err(read_cfg());

    let map: HashMap<String, (T, ShapeI)> =
        HashMap::deserialize(deserializer).map_err(serde::de::Error::custom)?;

    Ok(map
        .into_iter()
        .map(|(k, (v, s))| {
            (
                tf_to_annomap_key(k, cfg.as_ref().map(|cfg| cfg.current_prj_path())),
                (v, s),
            )
        })
        .collect())
}
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LabelMap<T>
where
    T: Serialize + DeserializeOwned,
{
    #[serde(flatten)]
    #[serde(serialize_with = "serialize_relative_paths")]
    #[serde(deserialize_with = "deserialize_relative_paths")]
    map: HashMap<String, (T, ShapeI)>,
}

impl<T> IntoIterator for LabelMap<T>
where
    T: Serialize + DeserializeOwned,
{
    type Item = (String, (T, ShapeI));
    type IntoIter = std::collections::hash_map::IntoIter<String, (T, ShapeI)>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
impl<T> FromIterator<(String, (T, ShapeI))> for LabelMap<T>
where
    T: Serialize + DeserializeOwned,
{
    fn from_iter<I: IntoIterator<Item = (String, (T, ShapeI))>>(iter: I) -> Self {
        Self {
            map: iter.into_iter().collect(),
        }
    }
}
impl<T> Index<&str> for LabelMap<T>
where
    T: Serialize + DeserializeOwned,
{
    type Output = (T, ShapeI);

    fn index(&self, index: &str) -> &Self::Output {
        &self.map[index]
    }
}
impl<T> LabelMap<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    pub fn insert(&mut self, key: String, value: (T, ShapeI)) {
        self.map.insert(key, value);
    }
    pub fn get_mut(&mut self, absolute_path: &str) -> Option<&mut (T, ShapeI)> {
        self.map.get_mut(absolute_path)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &(T, ShapeI))> {
        self.map.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut (T, ShapeI))> {
        self.map.iter_mut()
    }
    pub fn get(&self, key: &str) -> Option<&(T, ShapeI)> {
        self.map.get(key)
    }
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut (T, ShapeI)) -> bool,
    {
        self.map.retain(f);
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut (T, ShapeI)> {
        self.map.values_mut()
    }
    pub fn remove(&mut self, key: &str) {
        self.map.remove(key);
    }
}
