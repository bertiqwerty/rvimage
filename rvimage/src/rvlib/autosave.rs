use chrono::{Duration, Local, NaiveDate};
use regex::Regex;
use std::{
    fs,
    path::{Path, PathBuf},
};

use lazy_static::lazy_static;
use rvimage_domain::{to_rv, RvResult};

use crate::{file_util::osstr_to_str, result::trace_ok_err};

const DATE_FORMAT: &str = "%y%m%d";

pub const AUTOSAVE_KEEP_N_DAYS: i64 = 30;
pub const AUTOSAVE_INTERVAL_S: u64 = 120;

fn extract_date(filename: &str) -> Option<NaiveDate> {
    lazy_static! {
        static ref DATE_REGEX: Regex =
            Regex::new(r"autosave_d[0-9]{6}_").expect("Failed to compile regex");
    }
    if let Some(m) = DATE_REGEX.find(filename) {
        let m_str = m.as_str();
        let to_be_parsed = &m_str[10..m_str.len() - 1];
        return NaiveDate::parse_from_str(to_be_parsed, DATE_FORMAT).ok();
    }
    None
}

pub fn list_files(
    homefolder: Option<&Path>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
) -> RvResult<Vec<PathBuf>> {
    let mut res = vec![];
    if let Some(homefolder) = &homefolder {
        for entry in fs::read_dir(homefolder).map_err(to_rv)? {
            let entry = entry.map_err(to_rv)?;
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
                if let Some(date) = extract_date(filename) {
                    if start_date.map(|sd| date >= sd) != Some(false)
                        && end_date.map(|ed| date <= ed) != Some(false)
                    {
                        res.push(path);
                    }
                }
            }
        }
    }
    Ok(res)
}

pub fn make_timespan(n_days: i64) -> (NaiveDate, NaiveDate) {
    let today = Local::now().date_naive();
    let date_n_days_ago = today - Duration::days(n_days);
    (today, date_n_days_ago)
}

fn make_filepath(
    homefolder: Option<&Path>,
    prj_stem: &str,
    today_str: &str,
    n: u8,
) -> Option<PathBuf> {
    homefolder.map(|hf| hf.join(format!("{prj_stem}-autosave_d{today_str}_{n}.json")))
}

pub fn autosave(
    current_prj_path: &Path,
    homefolder: Option<String>,
    n_autosaves: u8,
    mut save_prj: impl FnMut(PathBuf) -> RvResult<()> + Send,
) -> RvResult<()> {
    let prj_stem = osstr_to_str(current_prj_path.file_stem())
        .map_err(to_rv)?
        .to_string();
    let homefolder = homefolder.map(PathBuf::from);

    let (now, date_n_days_ago) = make_timespan(AUTOSAVE_KEEP_N_DAYS);

    // remove too old files
    let files = list_files(homefolder.as_deref(), None, Some(date_n_days_ago))?;
    for p in files {
        tracing::info!("deleting {p:?}");
        trace_ok_err(fs::remove_file(p));
    }

    let today_str = now.format(DATE_FORMAT).to_string();

    let mf = |n| make_filepath(homefolder.as_deref(), &prj_stem, &today_str, n);
    for i in 1..(n_autosaves) {
        if let (Some(from), Some(to)) = (mf(i), mf(i - 1)) {
            if from.exists() {
                trace_ok_err(fs::copy(from, to));
            }
        }
    }
    let prj_path = mf(n_autosaves - 1);
    if let Some(prj_path) = prj_path {
        if trace_ok_err(save_prj(prj_path)).is_some() {
            tracing::info!("autosaved");
        }
    }
    Ok(())
}

#[cfg(test)]
use crate::{get_test_folder, tracing_setup::init_tracing_for_tests};

#[test]
fn test_extract_date() {
    init_tracing_for_tests();
    assert_eq!(
        extract_date("filename-autosave_d131214_0.json")
            .unwrap()
            .format("%y%m%d")
            .to_string(),
        "131214"
    );
    assert_eq!(extract_date("filename_d131214_0.json"), None);
    assert_eq!(extract_date("filename_123456.json"), None);
    assert_eq!(extract_date("filename_d123456.json"), None);
    assert_eq!(extract_date("filename_d123456_1.json"), None);
    assert_eq!(extract_date("filename"), None);
}

#[test]
fn test_list_files() {
    init_tracing_for_tests();
    let files = list_files(
        Some(&get_test_folder()),
        None,
        NaiveDate::parse_from_str("241216", DATE_FORMAT).ok(),
    );
    assert_eq!(
        files.unwrap()[0].file_name().unwrap(),
        Path::new("flower-autosave_d241215_1.json")
    );
}
