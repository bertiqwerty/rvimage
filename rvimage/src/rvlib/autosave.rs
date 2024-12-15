use chrono::{Duration, Local, NaiveDate};
use regex::Regex;
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
};

use lazy_static::lazy_static;
use rvimage_domain::{to_rv, RvResult};

use crate::{file_util::osstr_to_str, result::trace_ok_err};

const DATE_FORMAT: &str = "%y%m%d";

fn extract_date(filename: &str) -> Option<NaiveDate> {
    lazy_static! {
        static ref DATE_REGEX: Regex = Regex::new(r"_d[0-9]{6}_").expect("Failed to compile regex");
    }
    if let Some(m) = DATE_REGEX.find(filename) {
        let to_be_parsed = m.as_str().trim_matches('_').trim_start_matches('d');
        return NaiveDate::parse_from_str(to_be_parsed, DATE_FORMAT).ok();
    }
    None
}

fn list_files(homefolder: Option<&Path>, date_n_days_ago: NaiveDate) -> RvResult<Vec<PathBuf>> {
    let mut res = vec![];
    if let Some(homefolder) = &homefolder {
        for entry in fs::read_dir(homefolder).map_err(to_rv)? {
            let entry = entry.map_err(to_rv)?;
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|name| name.to_str()) {
                if let Some(date) = extract_date(filename) {
                    if date < date_n_days_ago {
                        res.push(path);
                    }
                }
            }
        }
    }
    Ok(res)
}

pub fn autosave(
    current_prj_path: &Path,
    homefolder: Option<String>,
    n_autosaves: u8,
    n_days: i64,
    mut save_prj: impl FnMut(PathBuf) -> RvResult<()>,
) -> RvResult<()> {
    let prj_stem = osstr_to_str(current_prj_path.file_stem())
        .map_err(to_rv)?
        .to_string();
    let homefolder = homefolder.map(PathBuf::from);
    let today = Local::now().date_naive();

    // remove too old files
    let n_days_ago = today - Duration::days(n_days);
    let files = list_files(homefolder.as_deref(), n_days_ago)?;
    for p in files {
        tracing::info!("deleting {p:?}");
        trace_ok_err(fs::remove_file(p));
    }

    let today_str = today.format(DATE_FORMAT).to_string();

    let make_filepath = move |n| {
        homefolder
            .clone()
            .map(|hf| hf.join(format!("{prj_stem}-autosave_d{today_str}_{n}.json")))
    };
    let mf_th = make_filepath.clone();
    thread::spawn(move || {
        for i in 0..(n_autosaves - 1) {
            if let (Some(from), Some(to)) = (mf_th(i), mf_th(i + 1)) {
                if from.exists() {
                    trace_ok_err(fs::copy(from, to));
                }
            }
        }
    });
    let prj_path = make_filepath(n_autosaves - 1);
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
        extract_date("filename_d131214_0.json")
            .unwrap()
            .format("%y%m%d")
            .to_string(),
        "131214"
    );
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
        NaiveDate::parse_from_str("241216", DATE_FORMAT).unwrap(),
    );
    assert_eq!(
        files.unwrap()[0].file_name().unwrap(),
        Path::new("flower-autosave_d241215_1.json")
    );
}