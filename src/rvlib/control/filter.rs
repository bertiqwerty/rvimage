use std::str::FromStr;

use exmex::prelude::*;
use exmex::{ops_factory, BinOp, ExError, MakeOperators, MatchLiteral, Operator};

use crate::result::{RvError, RvResult};
use crate::tools;
use crate::world::ToolsDataMap;

#[derive(Clone, Debug, Default)]
pub enum FilterPredicate {
    FilterStr(String),
    Label(Box<FilterPredicate>),
    Nolabel,
    And(Box<FilterPredicate>, Box<FilterPredicate>),
    Or(Box<FilterPredicate>, Box<FilterPredicate>),
    Not(Box<FilterPredicate>),
    #[default]
    TdmInjection,
}
impl FilterPredicate {
    pub fn apply(&self, path: &str, tdm: Option<&ToolsDataMap>) -> RvResult<bool> {
        Ok(match &self {
            FilterPredicate::FilterStr(s) => {
                if path.is_empty() {
                    true
                } else {
                    path.contains(s.trim())
                }
            }
            FilterPredicate::Label(label) => {
                let label = match &(**label) {
                    FilterPredicate::FilterStr(label) => label,
                    _ => Err(RvError::new("Label must be a string"))?,
                };
                let tdm = tdm.unwrap();
                if let Some(bbox_data) = tdm.get(tools::BBOX_NAME) {
                    if let Ok(specifics) = bbox_data.specifics.bbox() {
                        let labels = specifics.label_info.labels();
                        let annos = specifics.get_annos(path);
                        if let Some(annos) = annos {
                            annos
                                .cat_idxs()
                                .iter()
                                .any(|cat_idx| labels[*cat_idx].contains(label))
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            FilterPredicate::Nolabel => {
                if let Some(tdm) = tdm {
                    let bb_tool = tdm.get(tools::BBOX_NAME);
                    let annos = bb_tool
                        .and_then(|bbt| bbt.specifics.bbox().ok())
                        .and_then(|d| d.get_annos(path));
                    if let Some(annos) = annos {
                        annos.cat_idxs().is_empty()
                    } else {
                        true
                    }
                } else {
                    true
                }
            }
            FilterPredicate::And(a, b) => a.apply(path, tdm)? && b.apply(path, tdm)?,
            FilterPredicate::Or(a, b) => a.apply(path, tdm)? || b.apply(path, tdm)?,
            FilterPredicate::Not(a) => !a.apply(path, tdm)?,
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
    Operator::make_constant("nolabel", FilterPredicate::Nolabel)
);

#[derive(Clone, Debug, Default)]
pub struct PathMatcher;
impl MatchLiteral for PathMatcher {
    fn is_literal(text: &str) -> Option<&str> {
        let trimmed = text.trim();
        if trimmed.starts_with("label") || trimmed.starts_with("nolabel") {
            None
        } else {
            exmex::lazy_static::lazy_static! {
                static ref RE_VAR_NAME_EXACT: exmex::regex::Regex = exmex::regex::Regex::new(r"^[a-zA-z0-9\\/\-]+").unwrap();
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
    assert!(pred.apply("", None).unwrap());
    let s = "nolabel && (x || yy && zz)";
    let expr = FilterExpr::parse(s).unwrap();
    let pred = expr.eval(&[]).unwrap();
    let paths = ["ax", "by", "zzxx", "zxaxz", "yyasdzz3", "asd3yyz"];
    let expected = [false, false, true, false, true, false];
    for (path, expected) in paths.iter().zip(expected.iter()) {
        println!("path: {}", path);
        assert_eq!(pred.apply(path, None).unwrap(), *expected);
    }
}
