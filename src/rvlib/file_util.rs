#[cfg(feature = "azure_blob")]
use crate::cfg::AzureBlobCfg;
use crate::result::{to_rv, RvResult};
use crate::tools_data::{bbox_data, BboxExportData};
use crate::{
    cfg::{Cfg, PyHttpReaderCfg, SshCfg},
    rverr,
    world::ToolsDataMap,
};
use lazy_static::lazy_static;
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

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

pub const RVPRJ_PREFIX: &str = "rvprj_";

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
        .ok_or_else(|| rverr!("could not transform {:?} to &str", fname))
}

pub fn path_to_str(p: &Path) -> RvResult<&str> {
    osstr_to_str(Some(p.as_os_str()))
        .map_err(|e| rverr!("could not transform '{:?}' due to '{:?}'", p, e))
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
    osstr_to_str(p.file_stem())
        .map_err(|e| rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}

pub fn to_name_str(p: &Path) -> RvResult<&str> {
    osstr_to_str(p.file_name())
        .map_err(|e| rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub enum ConnectionData {
    Ssh(SshCfg),
    PyHttp(PyHttpReaderCfg),
    #[cfg(feature = "azure_blob")]
    AzureBlobCfg(AzureBlobCfg),
    #[default]
    None,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct MetaData {
    pub file_path: Option<String>,
    pub connection_data: ConnectionData,
    pub ssh_cfg: Option<SshCfg>,
    pub opened_folder: Option<String>,
    pub export_folder: Option<String>,
    pub is_loading_screen_active: Option<bool>,
}
impl MetaData {
    pub fn from_filepath(file_path: String) -> Self {
        MetaData {
            file_path: Some(file_path),
            connection_data: ConnectionData::None,
            ssh_cfg: None,
            opened_folder: None,
            export_folder: None,
            is_loading_screen_active: None,
        }
    }
}

pub const DEFAULT_PRJ_NAME: &str = "default";
pub fn is_prjname_set(prj_name: &str) -> bool {
    prj_name != DEFAULT_PRJ_NAME
}

pub fn make_prjcfg_filename(prj_name: &str) -> String {
    if prj_name.starts_with(RVPRJ_PREFIX) {
        format!("{prj_name}.json")
    } else {
        format!("{RVPRJ_PREFIX}{prj_name}.json")
    }
}
pub fn make_prjcfg_path(export_folder: &Path, prj_name: &str) -> PathBuf {
    let prj_name = match get_last_part_of_path(prj_name) {
        Some(prj_name_) => prj_name_.last_folder,
        None => prj_name,
    };
    Path::new(export_folder).join(make_prjcfg_filename(prj_name))
}
pub fn filename_to_prjname(filename: &str) -> RvResult<&str> {
    let file_name = osstr_to_str(Path::new(filename).file_stem()).map_err(to_rv)?;
    if file_name.starts_with(RVPRJ_PREFIX) {
        file_name.strip_prefix(RVPRJ_PREFIX).ok_or_else(|| {
            rverr!(
                "could not strip prefix '{}' from '{}'",
                RVPRJ_PREFIX,
                file_name
            )
        })
    } else {
        Ok(file_name)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ExportData {
    pub version: Option<String>,
    pub opened_folder: Option<String>,
    pub tools_data_map: ToolsDataMap,
    pub cfg: Cfg,
}
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ExportDataLegacy {
    pub opened_folder: Option<String>,
    pub bbox_data: Option<BboxExportData>,
    #[serde(skip_deserializing)]
    pub bbox_options: Option<bbox_data::Options>,
    pub cfg: Cfg,
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
