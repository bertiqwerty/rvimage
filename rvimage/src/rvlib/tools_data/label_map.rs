use std::{collections::HashMap, ops::Index};

use rvimage_domain::ShapeI;
use serde::{de::DeserializeOwned, Deserialize, Serialize, Serializer};

use crate::{cfg::read_cfg, file_util::tf_to_annomap_key, result::trace_ok_err};

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

    let map: HashMap<String, (T, ShapeI)> = HashMap::deserialize(deserializer)?;

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
