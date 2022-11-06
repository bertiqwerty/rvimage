use serde::{Deserialize, Serialize};
use serde_pickle::SerOptions;

use crate::result::{to_rv, RvError, RvResult};
use crate::tools::core::ConnectionData;
use crate::tools::MetaData;
use crate::tools_data::BboxToolData;
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

fn write(
    meta_data: &MetaData,
    bbox_specifics: BboxToolData,
    extension: &str,
    ser: fn(&BboxDataExport, &Path) -> RvResult<()>,
) -> RvResult<PathBuf> {
    let ef = meta_data
        .export_folder
        .as_ref()
        .ok_or_else(|| RvError::new("no export folder given"))?;
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
    let path = path::Path::new(ef).join(of).with_extension(extension);
    ser(&data, &path).map_err(to_rv)?;

    println!("exported labels to {:?}", path);
    Ok(path)
}

pub(super) fn write_json(meta_data: &MetaData, bbox_specifics: BboxToolData) -> RvResult<PathBuf> {
    let ser = |data: &BboxDataExport, path: &Path| {
        let data_str = serde_json::to_string(&data).map_err(to_rv)?;
        fs::write(&path, data_str).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "json", ser)
}

pub(super) fn write_pickle(
    meta_data: &MetaData,
    bbox_specifics: BboxToolData,
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
fn make_data(extension: &str) -> (BboxToolData, MetaData, PathBuf, &'static str) {
    let opened_folder = "xi".to_string();
    let test_export_folder = DEFAULT_TMPDIR.clone();
    if !test_export_folder.exists() {
        fs::create_dir(&test_export_folder).unwrap();
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
    let mut bbox_data = BboxToolData::new();
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
fn assert(key: &str, meta: MetaData, read: BboxDataExport, bbox_data: BboxToolData) {
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
    defer_file_removal!(path);
    let written_path = write_json(&meta, bbox_data.clone())?;
    let s = fs::read_to_string(written_path).map_err(to_rv)?;
    let read: BboxDataExport = serde_json::from_str(s.as_str()).map_err(to_rv)?;
    assert(key, meta, read, bbox_data);
    Ok(())
}
#[test]
fn test_pickle_export() -> RvResult<()> {
    let (bbox_data, meta, path, key) = make_data("pickle");
    defer_file_removal!(path);
    let written_path = write_pickle(&meta, bbox_data.clone())?;
    let f = File::open(written_path).map_err(to_rv)?;
    let read: BboxDataExport = serde_pickle::from_reader(f, DeOptions::new()).map_err(to_rv)?;
    assert(key, meta, read, bbox_data);
    Ok(())
}
