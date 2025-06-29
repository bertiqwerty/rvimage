use std::{
    collections::{btree_map, BTreeMap, HashMap},
    fmt::{Debug, Display},
    ops::Index,
    str::FromStr,
};

use rvimage_domain::{rverr, to_rv, RvResult, TPtF, TPtS};
use serde::{Deserialize, Serialize};

pub const PARAM_INTERVAL_SEPARATOR: &str = "-";
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
        let mut min_max_str_it = domain_str.trim().split(PARAM_INTERVAL_SEPARATOR);
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
