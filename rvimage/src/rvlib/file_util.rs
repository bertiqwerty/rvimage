use crate::{
    cfg::{CfgLegacy, CfgPrj},
    world::ToolsDataMap,
};
use lazy_static::lazy_static;
use rvimage_domain::{rverr, to_rv, RvResult};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::{
    collections::hash_map::DefaultHasher,
    ffi::OsStr,
    fmt::Debug,
    fs,
    hash::{Hash, Hasher},
    io,
    path::{Path, PathBuf},
};
use tracing::{error, info};

lazy_static! {
    pub static ref DEFAULT_TMPDIR: PathBuf = std::env::temp_dir().join("rvimage");
}
lazy_static! {
    pub static ref DEFAULT_HOMEDIR: PathBuf = match dirs::home_dir() {
        Some(p) => p.join(".rvimage"),
        _ => std::env::temp_dir().join("rvimage"),
    };
}
lazy_static! {
    pub static ref DEFAULT_PRJ_PATH: PathBuf =
        DEFAULT_HOMEDIR.join(DEFAULT_PRJ_NAME).join("default.rvi");
}

/// Keys of the annotation maps are the relative paths of the corresponding image files to the project folder.
pub fn tf_to_annomap_key(path: String, curr_prj_path: Option<&Path>) -> String {
    let path = path.replace('\\', "/");
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
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PathPair {
    path_absolute: String,
    path_relative: String,
}
impl PathPair {
    pub fn new(path_absolute: String, prj_path: &Path) -> Self {
        let path_absolute = path_absolute.replace('\\', "/");
        let prj_path = if prj_path == Path::new("") {
            None
        } else {
            Some(prj_path)
        };
        let path_relative = tf_to_annomap_key(path_absolute.clone(), prj_path);
        PathPair {
            path_absolute,
            path_relative,
        }
    }
    pub fn from_relative_path(path_relative: String, prj_path: &Path) -> Self {
        let path_absolute = if let Some(parent) = prj_path.parent() {
            let path_absolute = parent.join(path_relative.clone());
            if path_absolute.exists() {
                path_to_str(&path_absolute).unwrap().replace('\\', "/")
            } else {
                path_relative.replace('\\', "/")
            }
        } else {
            path_relative.replace('\\', "/")
        };
        PathPair {
            path_absolute,
            path_relative,
        }
    }
    pub fn path_absolute(&self) -> &str {
        &self.path_absolute
    }
    pub fn path_relative(&self) -> &str {
        &self.path_relative
    }
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub fn read_to_string<P>(p: P) -> RvResult<String>
where
    P: AsRef<Path> + Debug,
{
    fs::read_to_string(&p).map_err(|e| rverr!("could not read {:?} due to {:?}", p, e))
}
pub trait PixelEffect: FnMut(u32, u32) {}
impl<T: FnMut(u32, u32)> PixelEffect for T {}
pub fn filename_in_tmpdir(path: &str, tmpdir: &str) -> RvResult<String> {
    let path_hash = calculate_hash(&path);
    let path = PathBuf::from_str(path).unwrap();
    let fname = format!(
        "{path_hash}_{}",
        osstr_to_str(path.file_name()).map_err(to_rv)?
    );
    Path::new(tmpdir)
        .join(&fname)
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| rverr!("filename_in_tmpdir could not transform {:?} to &str", fname))
}

pub fn path_to_str(p: &Path) -> RvResult<&str> {
    osstr_to_str(Some(p.as_os_str()))
        .map_err(|e| rverr!("path_to_str could not transform '{:?}' due to '{:?}'", p, e))
}

pub fn osstr_to_str(p: Option<&OsStr>) -> io::Result<&str> {
    p.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{p:?} not found")))?
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{p:?} not convertible to unicode"),
            )
        })
}

pub fn to_stem_str(p: &Path) -> RvResult<&str> {
    let stem = p.file_stem();
    if stem.is_none() {
        Ok("")
    } else {
        osstr_to_str(stem)
            .map_err(|e| rverr!("to_stem_str could not transform '{:?}' due to '{:?}'", p, e))
    }
}

pub fn to_name_str(p: &Path) -> RvResult<&str> {
    osstr_to_str(p.file_name())
        .map_err(|e| rverr!("to_name_str could not transform '{:?}' due to '{:?}'", p, e))
}

pub const DEFAULT_PRJ_NAME: &str = "default";
pub fn is_prjname_set(prj_name: &str) -> bool {
    prj_name != DEFAULT_PRJ_NAME
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ExportData {
    pub version: Option<String>,
    pub tools_data_map: ToolsDataMap,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum SavedCfg {
    CfgPrj(CfgPrj),
    CfgLegacy(CfgLegacy),
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
        let _dfr = $crate::file_util::Defer { func: $f };
    };
}
pub fn checked_remove<'a, P: AsRef<Path> + Debug>(
    path: &'a P,
    func: fn(p: &'a P) -> io::Result<()>,
) {
    match func(path) {
        Ok(_) => info!("removed {path:?}"),
        Err(e) => error!("could not remove {path:?} due to {e:?}"),
    }
}
#[macro_export]
macro_rules! defer_folder_removal {
    ($path:expr) => {
        let func = || $crate::file_util::checked_remove($path, std::fs::remove_dir_all);
        $crate::defer!(func);
    };
}
#[macro_export]
macro_rules! defer_file_removal {
    ($path:expr) => {
        let func = || $crate::file_util::checked_remove($path, std::fs::remove_file);
        $crate::defer!(func);
    };
}

#[allow(clippy::needless_lifetimes)]
pub fn files_in_folder<'a>(
    folder: &'a str,
    prefix: &'a str,
    extension: &'a str,
) -> RvResult<impl Iterator<Item = PathBuf> + 'a> {
    Ok(fs::read_dir(folder)
        .map_err(|e| rverr!("could not open folder {} due to {}", folder, e))?
        .flatten()
        .map(|de| de.path())
        .filter(|p| {
            let prefix: &str = prefix; // Not sure why the borrow checker needs this.
            p.is_file()
                && if let Some(fname) = p.file_name() {
                    fname.to_str().unwrap().starts_with(prefix)
                } else {
                    false
                }
                && (p.extension() == Some(OsStr::new(extension)))
        }))
}

pub fn write<P, C>(path: P, contents: C) -> RvResult<()>
where
    P: AsRef<Path> + Debug,
    C: AsRef<[u8]>,
{
    fs::write(&path, contents).map_err(|e| rverr!("could not write to {:?} since {:?}", path, e))
}

#[macro_export]
macro_rules! p_to_rv {
    ($path:expr, $expr:expr) => {
        $expr.map_err(|e| format_rverr!("{:?}, failed on {e:?}", $path))
    };
}

pub struct LastPartOfPath<'a> {
    pub last_folder: &'a str,
    // will transform /a/b/c/ to /a/b/c
    pub path_wo_final_sep: &'a str,
    // offset is defined by " or ' that might by at the beginning and end of the path
    pub offset: usize,
    // ', ", or empty string depending on their existence
    pub mark: &'a str,
    // separators can be / on Linux or for http and \ on Windows
    pub n_removed_separators: usize,
}

impl<'a> LastPartOfPath<'a> {
    pub fn name(&self) -> String {
        format!(
            "{}{}{}",
            self.mark,
            self.last_folder.replace(':', "_"),
            self.mark
        )
    }
}

pub fn url_encode(url: &str) -> String {
    let mappings = [
        (" ", "%20"),
        ("+", "%2B"),
        (",", "%2C"),
        (";", "%3B"),
        ("*", "%2A"),
        ("(", "%28"),
        (")", "%29"),
    ];
    let mut url = url.replace(mappings[0].0, mappings[1].1);
    for m in mappings[1..].iter() {
        url = url
            .replace(m.0, m.1)
            .replace(m.1.to_lowercase().as_str(), m.1);
    }
    url
}

fn get_last_part_of_path_by_sep(path: &str, sep: char) -> Option<LastPartOfPath> {
    if path.contains(sep) {
        let mark = if path.starts_with('\'') && path.ends_with('\'') {
            "\'"
        } else if path.starts_with('"') && path.ends_with('"') {
            "\""
        } else {
            ""
        };
        let offset = mark.len();
        let mut path_wo_final_sep = &path[offset..(path.len() - offset)];
        let n_fp_slice_initial = path_wo_final_sep.len();
        let mut last_folder = path_wo_final_sep.split(sep).last().unwrap_or("");
        while last_folder.is_empty() && !path_wo_final_sep.is_empty() {
            path_wo_final_sep = &path_wo_final_sep[0..path_wo_final_sep.len() - 1];
            last_folder = path_wo_final_sep.split(sep).last().unwrap_or("");
        }
        Some(LastPartOfPath {
            last_folder,
            path_wo_final_sep,
            offset,
            mark,
            n_removed_separators: n_fp_slice_initial - path_wo_final_sep.len(),
        })
    } else {
        None
    }
}

pub fn get_last_part_of_path(path: &str) -> Option<LastPartOfPath> {
    let lp_fw = get_last_part_of_path_by_sep(path, '/');
    if let Some(lp) = &lp_fw {
        if let Some(lp_fwbw) = get_last_part_of_path_by_sep(lp.last_folder, '\\') {
            Some(lp_fwbw)
        } else {
            lp_fw
        }
    } else {
        get_last_part_of_path_by_sep(path, '\\')
    }
}

pub fn get_prj_name<'a>(prj_path: &'a Path, opened_folder: Option<&'a str>) -> &'a str {
    let default_prjname = if let Some(of) = opened_folder {
        of
    } else {
        DEFAULT_PRJ_NAME
    };
    osstr_to_str(prj_path.file_stem()).unwrap_or(default_prjname)
}

pub fn local_file_info<P>(p: P) -> String
where
    P: AsRef<Path>,
{
    fs::metadata(p)
        .map(|md| {
            let n_bytes = md.len();
            if n_bytes < 1024 {
                format!("{}b", md.len())
            } else if n_bytes < 1024u64.pow(2) {
                format!("{:.3}kb", md.len() as f64 / 1024f64)
            } else {
                format!("{:.3}mb", md.len() as f64 / 1024f64.powi(2))
            }
        })
        .unwrap_or_else(|_| "".to_string())
}

pub fn get_test_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data")
}

#[test]
fn get_last_part() {
    let path = "http://localhost:8000/a/21%20%20b/Beg.png";
    let lp = get_last_part_of_path(path).unwrap();
    assert_eq!(lp.last_folder, "Beg.png");
}

#[test]
fn last_folder_part() {
    assert_eq!(
        get_last_part_of_path("a/b/c").map(|lp| lp.name()),
        Some("c".to_string())
    );
    assert_eq!(
        get_last_part_of_path_by_sep("a/b/c", '\\').map(|lp| lp.name()),
        None
    );
    assert_eq!(
        get_last_part_of_path_by_sep("a\\b\\c", '/').map(|lp| lp.name()),
        None
    );
    assert_eq!(
        get_last_part_of_path("a\\b\\c").map(|lp| lp.name()),
        Some("c".to_string())
    );
    assert_eq!(get_last_part_of_path("").map(|lp| lp.name()), None);
    assert_eq!(
        get_last_part_of_path("a/b/c/").map(|lp| lp.name()),
        Some("c".to_string())
    );
    assert_eq!(
        get_last_part_of_path("aadfh//bdafl////aksjc/////").map(|lp| lp.name()),
        Some("aksjc".to_string())
    );
    assert_eq!(
        get_last_part_of_path("\"aa dfh//bdafl////aks jc/////\"").map(|lp| lp.name()),
        Some("\"aks jc\"".to_string())
    );
    assert_eq!(
        get_last_part_of_path("'aa dfh//bdafl////aks jc/////'").map(|lp| lp.name()),
        Some("'aks jc'".to_string())
    );
}

#[cfg(target_family = "windows")]
#[test]
fn test_stem() {
    assert_eq!(to_stem_str(Path::new("a/b/c.png")).unwrap(), "c");
    assert_eq!(to_stem_str(Path::new("c:\\c.png")).unwrap(), "c");
    assert_eq!(to_stem_str(Path::new("c:\\")).unwrap(), "");
}
#[cfg(target_family = "unix")]
#[test]
fn test_stem() {
    assert_eq!(to_stem_str(Path::new("a/b/c.png")).unwrap(), "c");
    assert_eq!(to_stem_str(Path::new("c:\\c.png")).unwrap(), "c:\\c");
    assert_eq!(to_stem_str(Path::new("/c.png")).unwrap(), "c");
    assert_eq!(to_stem_str(Path::new("/")).unwrap(), "");
}

#[test]
fn test_pathpair() {
    fn test(
        path: &str,
        prj_path: &str,
        expected_absolute: &str,
        expected_relative: &str,
        skip_from_relative: bool,
    ) {
        let pp = PathPair::new(path.to_string(), Path::new(prj_path));
        assert_eq!(pp.path_absolute(), expected_absolute);
        assert_eq!(pp.path_relative(), expected_relative);
        if !skip_from_relative {
            let pp =
                PathPair::from_relative_path(expected_relative.to_string(), Path::new(prj_path));
            assert_eq!(pp.path_absolute(), expected_absolute);
            assert_eq!(pp.path_relative(), expected_relative);
        }
    }

    let relative_path = "somesubfolder/notanimage.png";
    let prj_path_p = get_test_folder().join("rvprj_v3-3_test_dummy.rvi");
    let prj_path_parent_p = prj_path_p.parent().unwrap();
    let path_p = prj_path_parent_p.join(relative_path);
    let prj_path = path_to_str(prj_path_p.as_path()).unwrap();
    let path = path_to_str(path_p.as_path()).unwrap();
    test(
        path,
        prj_path,
        &path.replace("\\", "/"),
        relative_path,
        false,
    );

    #[cfg(target_family = "windows")]
    {
        let prj_path = "a\\b\\c\\prj.rvi";
        let path = "a\\b\\c\\d\\e.png";
        test(path, prj_path, &path.replace("\\", "/"), "d/e.png", true);
    }
}
