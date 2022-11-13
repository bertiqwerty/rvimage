use serde_pickle::{DeOptions, SerOptions};

use crate::annotations::BboxAnnotations;
use crate::file_util::{BboxDataExport, ExportData, MetaData};
use crate::format_rverr;
use crate::result::{to_rv, RvError, RvResult};
use crate::tools_data::BboxSpecificData;
use std::collections::HashMap;
use std::fs;
use std::fs::File;

use std::path::{self, Path, PathBuf};

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
    ser: fn(&ExportData, &Path) -> RvResult<()>,
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
    let data = ExportData {
        opened_folder: of.clone(),
        connection_data: meta_data.connection_data.clone(),
        bbox_data: Some(BboxDataExport {
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
        }),
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
    let ser = |data: &ExportData, path: &Path| {
        let data_str = serde_json::to_string(&data).map_err(to_rv)?;
        fs::write(&path, data_str).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "json", ser)
}

fn _convert_read(read: ExportData) -> RvResult<BboxSpecificData> {
    let bb_read = read
        .bbox_data
        .ok_or_else(|| RvError::new("import does not contain bbox data"))?;
    let mut bbox_data = BboxSpecificData::new();
    for (lab, clr) in bb_read.labels.into_iter().zip(bb_read.colors.into_iter()) {
        bbox_data.push(lab, Some(clr));
    }
    bbox_data.remove_cat(0);
    bbox_data.set_annotations_map(
        bb_read
            .annotations
            .into_iter()
            .map(|(s, (bbs, cat_ids))| (s, BboxAnnotations::from_bbs_cats(bbs, cat_ids)))
            .collect(),
    )?;
    Ok(bbox_data)
}

pub(super) fn _read_json(filename: &str) -> RvResult<BboxSpecificData> {
    let s = fs::read_to_string(filename).map_err(to_rv)?;
    let read: ExportData = serde_json::from_str(s.as_str()).map_err(to_rv)?;
    _convert_read(read)
}

pub(super) fn write_pickle(
    meta_data: &MetaData,
    bbox_specifics: BboxSpecificData,
) -> RvResult<PathBuf> {
    let ser = |data: &ExportData, path: &Path| {
        let mut file = File::create(path).map_err(to_rv)?;
        serde_pickle::to_writer(&mut file, data, SerOptions::new()).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "pickle", ser)
}

pub(super) fn _read_pickle(filename: &str) -> RvResult<BboxSpecificData> {
    let f = File::open(filename).map_err(to_rv)?;
    let read: ExportData = serde_pickle::from_reader(f, DeOptions::new()).map_err(to_rv)?;
    _convert_read(read)
}
#[cfg(test)]
use {
    super::core::make_test_bbs,
    crate::cfg::SshCfg,
    crate::file_util::osstr_to_str,
    crate::{defer_file_removal, file_util::DEFAULT_TMPDIR},
};
#[cfg(test)]
fn make_data(extension: &str) -> (BboxSpecificData, MetaData, PathBuf) {
    use crate::file_util::ConnectionData;

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
    (bbox_data, meta, test_export_path)
}
#[test]
fn test_json_export() -> RvResult<()> {
    let (bbox_data, meta, path) = make_data("json");
    defer_file_removal!(&path);
    let written_path = write_json(&meta, bbox_data.clone())?;
    let bbox_data_read = _read_json(osstr_to_str(Some(written_path.as_os_str())).map_err(to_rv)?)?;
    assert_eq!(bbox_data, bbox_data_read);
    Ok(())
}
#[test]
fn test_pickle_export() -> RvResult<()> {
    let (bbox_data, meta, path) = make_data("pickle");
    defer_file_removal!(&path);
    let written_path = write_pickle(&meta, bbox_data.clone())?;
    let bbox_data_read =
        _read_pickle(osstr_to_str(Some(written_path.as_os_str())).map_err(to_rv)?)?;
    assert_eq!(bbox_data, bbox_data_read);
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
