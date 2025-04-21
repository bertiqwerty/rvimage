use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub fn wrap_if<T>(x: T, cond: bool) -> Option<T> {
    if cond {
        Some(x)
    } else {
        None
    }
}

#[allow(clippy::needless_lifetimes)]
fn xor_mask<'a>(mask: &'a [bool], other: bool) -> impl Iterator<Item = usize> + Clone + 'a {
    let res = mask
        .iter()
        .enumerate()
        .filter(move |(_, is_selected)| other ^ **is_selected)
        .map(|(i, _)| i);
    res
}

#[allow(clippy::needless_lifetimes)]
pub fn true_indices<'a>(mask: &'a [bool]) -> impl Iterator<Item = usize> + Clone + 'a {
    xor_mask(mask, false)
}

pub fn natural_cmp(s1: &str, s2: &str) -> Ordering {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"(\d+)").unwrap();
    }
    let mut idx = 0;
    while idx < s1.len().min(s2.len()) {
        let c1 = s1.chars().nth(idx).unwrap();
        let c2 = s2.chars().nth(idx).unwrap();
        if c1.is_ascii_digit() && c2.is_ascii_digit() {
            let n1 = RE.captures(&s1[idx..]).unwrap()[0]
                .parse::<usize>()
                .unwrap();
            let n2 = RE.captures(&s2[idx..]).unwrap()[0]
                .parse::<usize>()
                .unwrap();
            if n1 != n2 {
                return n1.cmp(&n2);
            }
            idx += n1.to_string().len();
        } else {
            if c1 != c2 {
                return c1.cmp(&c2);
            }
            idx += 1;
        }
    }
    s1.len().cmp(&s2.len())
}
pub fn version_label() -> String {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const GIT_DESC: &str = env!("GIT_DESC");
    #[allow(clippy::const_is_empty)]
    if GIT_DESC.is_empty() {
        format!("Version {VERSION}")
    } else {
        const GIT_DIRTY: &str = env!("GIT_DIRTY");
        let is_dirty = GIT_DIRTY == "true";
        format!(
            "Version {}{}\n",
            &GIT_DESC,
            if is_dirty { " DIRTY" } else { "" }
        )
    }
}

#[macro_export]
macro_rules! measure_time {
    ($name:expr, $block:expr) => {{
        // let start = std::time::Instant::now();
        let result = $block;
        // tracing::warn!("{} took {} millis", $name, start.elapsed().as_millis());
        result
    }};
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum Visibility {
    All,
    None,
    // contains index of label that is to be shown exclusively
    Only(usize),
}
#[test]
fn test_natural_sort() {
    assert_eq!(natural_cmp("s10", "s2"), Ordering::Greater);
    assert_eq!(natural_cmp("10s", "s2"), Ordering::Less);
    assert_eq!(natural_cmp("10", "2"), Ordering::Greater);
    assert_eq!(natural_cmp("10.0", "10.0"), Ordering::Equal);
    assert_eq!(natural_cmp("20.0", "10.0"), Ordering::Greater);
    assert_eq!(
        natural_cmp("a lot of text 20.0 .", "a lot of text 100.0"),
        Ordering::Less
    );
    assert_eq!(
        natural_cmp("a lot of 7text 20.0 .", "a lot of 3text 100.0"),
        Ordering::Greater
    );
}

pub struct Defer<F: FnMut()> {
    pub func: F,
}
impl<F: FnMut()> Drop for Defer<F> {
    fn drop(&mut self) {
        (self.func)();
    }
}
#[macro_export]
macro_rules! defer {
    ($f:expr) => {
        let _dfr = $crate::Defer { func: $f };
    };
}
#[macro_export]
macro_rules! time_scope {
    ($name:expr) => {
        let now = std::time::Instant::now();
        #[cfg(feature = "print_timings")]
        let f = || eprintln!("{} {}", $name, now.elapsed().as_micros());
        #[cfg(not(feature = "print_timings"))]
        let f = || ();
        $crate::defer!(f);
    };
}
