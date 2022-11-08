use serde::{Deserialize, Serialize};
use serde_pickle::SerOptions;

use crate::format_rverr;
use crate::result::{to_rv, RvError, RvResult};
use crate::tools::core::ConnectionData;
use crate::tools::MetaData;
use crate::tools_data::BboxSpecificData;
use crate::util::BB;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::fs::File;

use std::path::{self, Path, PathBuf};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
struct BboxDataExport {
    pub opened_folder: String,
    pub connection_data: ConnectionData,
    pub labels: Vec<String>,
    pub colors: Vec<[u8; 3]>,
    pub annotations: HashMap<String, (Vec<BB>, Vec<usize>)>,
}
fn get_last_part_of_path(path: &str, sep: char) -> Option<String> {
    if path.contains(sep) {
        let mark = if path.starts_with('\'') && path.ends_with('\'') {
            "\'"
        } else if path.starts_with('"') && path.ends_with('"') {
            "\""
        } else {
            ""
        };
        let offset = mark.len();
        let mut fp_slice = &path[offset..(path.len() - offset)];
        let mut last_folder = fp_slice.split(sep).last().unwrap_or("");
        while last_folder.is_empty() && !fp_slice.is_empty() {
            fp_slice = &fp_slice[0..fp_slice.len() - 1];
            last_folder = fp_slice.split(sep).last().unwrap_or("");
        }
        Some(format!("{}{}{}", mark, last_folder, mark))
    } else {
        None
    }
}

fn write(
    meta_data: &MetaData,
    bbox_specifics: BboxSpecificData,
    extension: &str,
    ser: fn(&BboxDataExport, &Path) -> RvResult<()>,
) -> RvResult<PathBuf> {
    let ef = meta_data
        .export_folder
        .as_ref()
        .ok_or_else(|| RvError::new("no export folder given"))?;
    let ef_path = Path::new(ef);
    match fs::create_dir_all(ef_path) {
        Ok(_) => Ok(()),
        Err(e) => Err(format_rverr!(
            "could not create {:?} due to {:?}",
            ef_path,
            e
        )),
    }?;

    let of = meta_data
        .opened_folder
        .as_ref()
        .ok_or_else(|| RvError::new("no folder opened"))?;
    let data = BboxDataExport {
        opened_folder: of.clone(),
        connection_data: meta_data.connection_data.clone(),
        labels: bbox_specifics.labels().clone(),
        colors: bbox_specifics.colors().clone(),
        annotations: bbox_specifics
            .anno_iter()
            .map(|(filename, annos)| {
                (
                    filename.clone(),
                    (annos.bbs().clone(), annos.cat_ids().clone()),
                )
            })
            .collect::<HashMap<_, _>>(),
    };
    let of_last_part_linux = get_last_part_of_path(of, '/');
    let of_last_part_windows =
        get_last_part_of_path(of_last_part_linux.as_ref().unwrap_or(of), '\\');
    let of_last_part =
        of_last_part_windows.unwrap_or_else(|| of_last_part_linux.unwrap_or_else(|| of.clone()));
    let path = path::Path::new(ef_path)
        .join(of_last_part)
        .with_extension(extension);
    ser(&data, &path).map_err(to_rv)?;

    println!("exported labels to {:?}", path);
    Ok(path)
}

pub(super) fn write_json(
    meta_data: &MetaData,
    bbox_specifics: BboxSpecificData,
) -> RvResult<PathBuf> {
    let ser = |data: &BboxDataExport, path: &Path| {
        let data_str = serde_json::to_string(&data).map_err(to_rv)?;
        fs::write(&path, data_str).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "json", ser)
}

pub(super) fn write_pickle(
    meta_data: &MetaData,
    bbox_specifics: BboxSpecificData,
) -> RvResult<PathBuf> {
    let ser = |data: &BboxDataExport, path: &Path| {
        let mut file = File::create(path).map_err(to_rv)?;
        serde_pickle::to_writer(&mut file, data, SerOptions::new()).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "pickle", ser)
}

#[cfg(test)]
use {
    super::core::make_test_bbs,
    crate::cfg::SshCfg,
    crate::{defer_file_removal, util::DEFAULT_TMPDIR},
    serde_pickle::DeOptions,
};
#[cfg(test)]
fn make_data(extension: &str) -> (BboxSpecificData, MetaData, PathBuf, &'static str) {
    let opened_folder = "xi".to_string();
    let test_export_folder = DEFAULT_TMPDIR.clone();

    match fs::create_dir(&test_export_folder) {
        Ok(_) => (),
        Err(e) => {
            println!("{:?}", e);
        }
    }

    let test_export_path = DEFAULT_TMPDIR.join(format!("{}.{}", opened_folder, extension));
    let mut meta = MetaData::from_filepath(
        test_export_path
            .with_extension("egal")
            .to_str()
            .unwrap()
            .to_string(),
    );
    meta.opened_folder = Some(opened_folder);
    meta.export_folder = Some(test_export_folder.to_str().unwrap().to_string());
    meta.connection_data = ConnectionData::Ssh(SshCfg::default());
    let mut bbox_data = BboxSpecificData::new();
    bbox_data.push("x".to_string(), None);
    bbox_data.remove_cat(0);
    let mut bbs = make_test_bbs();
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    let key = "dummyfile";
    let annos = bbox_data.get_annos_mut(key);
    for bb in bbs {
        annos.add_bb(bb, 0);
    }
    (bbox_data, meta, test_export_path, key)
}
#[cfg(test)]
fn assert(key: &str, meta: MetaData, read: BboxDataExport, bbox_data: BboxSpecificData) {
    assert_eq!(read.opened_folder, meta.opened_folder.unwrap());
    assert_eq!(read.connection_data, ConnectionData::Ssh(SshCfg::default()));
    assert_eq!(read.labels, bbox_data.labels().clone());
    assert_eq!(read.colors, bbox_data.colors().clone());
    assert_eq!(&read.annotations[key].0, bbox_data.get_annos(key).bbs());
    assert_eq!(&read.annotations[key].1, bbox_data.get_annos(key).cat_ids());
}
#[test]
fn test_json_export() -> RvResult<()> {
    let (bbox_data, meta, path, key) = make_data("json");
    defer_file_removal!(&path);
    let written_path = write_json(&meta, bbox_data.clone())?;
    let s = fs::read_to_string(written_path).map_err(to_rv)?;
    let read: BboxDataExport = serde_json::from_str(s.as_str()).map_err(to_rv)?;
    assert(key, meta, read, bbox_data);
    Ok(())
}
#[test]
fn test_pickle_export() -> RvResult<()> {
    let (bbox_data, meta, path, key) = make_data("pickle");
    defer_file_removal!(&path);
    let written_path = write_pickle(&meta, bbox_data.clone())?;
    let f = File::open(written_path).map_err(to_rv)?;
    let read: BboxDataExport = serde_pickle::from_reader(f, DeOptions::new()).map_err(to_rv)?;
    assert(key, meta, read, bbox_data);
    Ok(())
}
#[test]
fn last_folder_part() {
    assert_eq!(get_last_part_of_path("a/b/c", '/'), Some("c".to_string()));
    assert_eq!(get_last_part_of_path("a/b/c", '\\'), None);
    assert_eq!(get_last_part_of_path("a\\b\\c", '/'), None);
    assert_eq!(
        get_last_part_of_path("a\\b\\c", '\\'),
        Some("c".to_string())
    );
    assert_eq!(get_last_part_of_path("", '/'), None);
    assert_eq!(get_last_part_of_path("a/b/c/", '/'), Some("c".to_string()));
    assert_eq!(
        get_last_part_of_path("aadfh//bdafl////aksjc/////", '/'),
        Some("aksjc".to_string())
    );
    assert_eq!(
        get_last_part_of_path("\"aa dfh//bdafl////aks jc/////\"", '/'),
        Some("\"aks jc\"".to_string())
    );
    assert_eq!(
        get_last_part_of_path("'aa dfh//bdafl////aks jc/////'", '/'),
        Some("'aks jc'".to_string())
    );
}
