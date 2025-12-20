use std::str::FromStr;

use exmex::prelude::*;
use exmex::{BinOp, ExError, MakeOperators, MatchLiteral, Operator, ops_factory};

use crate::result::ignore_error;
use crate::tools::ATTRIBUTES_NAME;
use crate::tools_data::annotations::InstanceAnnotations;
use crate::tools_data::parameters::PARAM_INTERVAL_SEPARATOR;
use crate::tools_data::{Annotate, InstanceAnnotate, ToolSpecifics};
use crate::tools_data::{LabelInfo, ToolsDataMap};

use rvimage_domain::{RvError, RvResult, rverr};

fn contains_label<T>(
    label: &str,
    label_info: &LabelInfo,
    annos: Option<&InstanceAnnotations<T>>,
) -> bool
where
    T: InstanceAnnotate + PartialEq + Clone + Default,
{
    if let Some(annos) = annos {
        annos
            .cat_idxs()
            .iter()
            .any(|cat_idx| label_info.labels()[*cat_idx].contains(label))
    } else {
        false
    }
}
fn has_any_label<T>(annos: Option<&InstanceAnnotations<T>>) -> bool
where
    T: InstanceAnnotate + PartialEq + Clone + Default,
{
    if let Some(annos) = annos {
        !annos.elts().is_empty()
    } else {
        false
    }
}

#[derive(Clone, Debug, Default)]
pub enum FilterPredicate {
    FilterStr(String),
    Label(Box<FilterPredicate>),
    Tool(Box<FilterPredicate>),
    Attribute(Box<FilterPredicate>),
    Nolabel,
    Anylabel,
    And(Box<FilterPredicate>, Box<FilterPredicate>),
    Or(Box<FilterPredicate>, Box<FilterPredicate>),
    Not(Box<FilterPredicate>),
    #[default]
    TdmInjection,
}
impl FilterPredicate {
    pub fn apply(
        &self,
        path: &str,
        tdm: Option<&ToolsDataMap>,
        active_tool_name: Option<&str>,
    ) -> RvResult<bool> {
        Ok(match &self {
            FilterPredicate::FilterStr(s) => {
                if path.is_empty() {
                    true
                } else {
                    path.contains(s.trim())
                }
            }
            FilterPredicate::Attribute(attr_str) => {
                let attr_str = match &(**attr_str) {
                    FilterPredicate::FilterStr(s) => s,
                    _ => Err(RvError::new("Label must be a string"))?,
                };
                let attr_tuple = attr_str.split_once(':').ok_or(rverr!(
                    "Attribute must be of the form <attr_name>:<attr_val>, found {}",
                    attr_str
                ))?;
                let (attr_name, attr_val_str) = attr_tuple;
                let mut found = false;
                if let Some(tdm) = tdm
                    && let Some(data) = tdm.get(ATTRIBUTES_NAME)
                    && let Some(attr_val) =
                        ignore_error(data.specifics.attributes()).and_then(|d| {
                            d.get_annos(path)
                                .and_then(|annos| annos.get(attr_name.trim()))
                        })
                {
                    if attr_val_str.contains(PARAM_INTERVAL_SEPARATOR) {
                        match attr_val.in_domain_str(attr_val_str.trim()) {
                            Ok(b) => {
                                found = b;
                            }
                            Err(_) => {
                                found = attr_val.corresponds_to_str(attr_val_str.trim())?;
                            }
                        }
                    } else {
                        found = attr_val.corresponds_to_str(attr_val_str.trim())?;
                    }
                }

                found
            }
            FilterPredicate::Label(label) => {
                let label = match &(**label) {
                    FilterPredicate::FilterStr(label) => label,
                    _ => Err(RvError::new("Label must be a string"))?,
                };
                if let (Some(tdm), Some(active_tool_name)) = (tdm, active_tool_name) {
                    if let Some(data) = tdm.get(active_tool_name) {
                        data.specifics.apply(
                            |d| Ok(contains_label(label, &d.label_info, d.get_annos(path))),
                            |d| Ok(contains_label(label, &d.label_info, d.get_annos(path))),
                        ) == Ok(true)
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
            FilterPredicate::Tool(tool_name) => {
                let tool_name = match &(**tool_name) {
                    FilterPredicate::FilterStr(tool) => tool,
                    _ => Err(RvError::new("Label must be a string"))?,
                };
                if let Some(tdm) = tdm {
                    if let Some(data) = tdm.get(tool_name) {
                        match &data.specifics {
                            ToolSpecifics::Attributes(d) => d.has_annos(path),
                            ToolSpecifics::Bbox(d) => d.has_annos(path),
                            ToolSpecifics::Brush(d) => d.has_annos(path),
                            ToolSpecifics::Rot90(d) => d.has_annos(path),
                            _ => false,
                        }
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
            FilterPredicate::Anylabel => {
                if let (Some(tdm), Some(active_tool_name)) = (tdm, active_tool_name) {
                    let data = tdm.get(active_tool_name);
                    data.and_then(|data| {
                        data.specifics
                            .apply(
                                |d| Ok(has_any_label(d.get_annos(path))),
                                |d| Ok(has_any_label(d.get_annos(path))),
                            )
                            .ok()
                    }) == Some(true)
                } else {
                    false
                }
            }
            FilterPredicate::Nolabel => {
                if let (Some(tdm), Some(active_tool_name)) = (tdm, active_tool_name) {
                    let data = tdm.get(active_tool_name);
                    data.and_then(|data| {
                        data.specifics
                            .apply(
                                |d| Ok(has_any_label(d.get_annos(path))),
                                |d| Ok(has_any_label(d.get_annos(path))),
                            )
                            .ok()
                    }) == Some(false)
                } else {
                    true
                }
            }
            FilterPredicate::And(a, b) => {
                a.apply(path, tdm, active_tool_name)? && b.apply(path, tdm, active_tool_name)?
            }
            FilterPredicate::Or(a, b) => {
                a.apply(path, tdm, active_tool_name)? || b.apply(path, tdm, active_tool_name)?
            }
            FilterPredicate::Not(a) => !a.apply(path, tdm, active_tool_name)?,
            FilterPredicate::TdmInjection => true,
        })
    }
}
impl FromStr for FilterPredicate {
    type Err = ExError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(FilterPredicate::FilterStr(s.to_string()))
    }
}

ops_factory!(
    FilterPredicateFactory,
    FilterPredicate,
    Operator::make_bin(
        "||",
        BinOp {
            apply: |a: FilterPredicate, b: FilterPredicate| {
                FilterPredicate::Or(Box::new(a), Box::new(b))
            },
            prio: 1,
            is_commutative: true
        }
    ),
    Operator::make_bin(
        "&&",
        BinOp {
            apply: |a: FilterPredicate, b: FilterPredicate| {
                FilterPredicate::And(Box::new(a), Box::new(b))
            },
            prio: 1,
            is_commutative: true
        }
    ),
    Operator::make_unary("!", |a: FilterPredicate| FilterPredicate::Not(Box::new(a))),
    Operator::make_unary("label", |a: FilterPredicate| FilterPredicate::Label(
        Box::new(a)
    )),
    Operator::make_unary("attr", |a: FilterPredicate| FilterPredicate::Attribute(
        Box::new(a)
    )),
    Operator::make_unary("tool", |a: FilterPredicate| FilterPredicate::Tool(
        Box::new(a)
    )),
    Operator::make_constant("nolabel", FilterPredicate::Nolabel),
    Operator::make_constant("anylabel", FilterPredicate::Anylabel)
);

#[derive(Clone, Debug, Default)]
pub struct PathMatcher;
impl MatchLiteral for PathMatcher {
    fn is_literal(text: &str) -> Option<&str> {
        let trimmed = text.trim();
        if trimmed.starts_with("label")
            || trimmed.starts_with("nolabel")
            || trimmed.starts_with("anylabel")
            || trimmed.starts_with("attr")
            || trimmed.starts_with("tool")
        {
            None
        } else {
            exmex::lazy_static::lazy_static! {
                static ref RE_VAR_NAME_EXACT: exmex::regex::Regex = exmex::regex::Regex::new(r"^[a-zA-z0-9\\/\-:. ]+").unwrap();
            }
            RE_VAR_NAME_EXACT.find(text).map(|m| m.as_str())
        }
    }
}

pub type FilterExpr = FlatEx<FilterPredicate, FilterPredicateFactory, PathMatcher>;

#[test]
fn test_filter_exmex() {
    let s = "nolabel";
    let expr = FilterExpr::parse(s).unwrap();
    let pred = expr.eval(&[]).unwrap();
    assert!(pred.apply("", None, None).unwrap());
    let s = "nolabel && (x || yy && zz)";
    let expr = FilterExpr::parse(s).unwrap();
    let pred = expr.eval(&[]).unwrap();
    let paths = ["ax", "by", "zzxx", "zxaxz", "yyasdzz3", "asd3yyz"];
    let expected = [false, false, true, false, true, false];
    for (path, expected) in paths.iter().zip(expected.iter()) {
        println!("path: {}", path);
        assert_eq!(pred.apply(path, None, None).unwrap(), *expected);
    }
}
