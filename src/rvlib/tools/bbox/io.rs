use serde::{Deserialize, Serialize};
use serde_pickle::SerOptions;

use crate::cfg::SshCfg;
use crate::result::{to_rv, RvError, RvResult};
use crate::tools::MetaData;
use crate::tools_data::BboxToolData;
use std::fmt::Debug;
use std::fs;
use std::fs::File;

use std::path::{self, Path, PathBuf};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
struct BboxDataIo {
    pub opened_folder: String,
    pub ssh_cfg: Option<SshCfg>,
    pub bbox_data: BboxToolData,
}

fn write(
    meta_data: &MetaData,
    bbox_specifics: BboxToolData,
    extension: &str,
    ser: fn(&BboxDataIo, &Path) -> RvResult<()>,
) -> RvResult<PathBuf> {
    let ef = meta_data
        .export_folder
        .as_ref()
        .ok_or_else(|| RvError::new("no export folder given"))?;
    let of = meta_data
        .opened_folder
        .as_ref()
        .ok_or_else(|| RvError::new("no folder opened"))?;

    let data = BboxDataIo {
        opened_folder: of.clone(),
        ssh_cfg: meta_data.ssh_cfg.clone(),
        bbox_data: bbox_specifics,
    };
    let path = path::Path::new(ef).join(of).with_extension(extension);
    ser(&data, &path).map_err(to_rv)?;

    println!("exported labels to {:?}", path);
    Ok(path)
}

pub(super) fn write_json(meta_data: &MetaData, bbox_specifics: BboxToolData) -> RvResult<PathBuf> {
    let ser = |data: &BboxDataIo, path: &Path| {
        let data_str = serde_json::to_string_pretty(&data).map_err(to_rv)?;
        fs::write(&path, data_str).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "json", ser)
}

pub(super) fn write_pickle(
    meta_data: &MetaData,
    bbox_specifics: BboxToolData,
) -> RvResult<PathBuf> {
    let ser = |data: &BboxDataIo, path: &Path| {
        let mut file = File::create(path).map_err(to_rv)?;
        serde_pickle::to_writer(&mut file, data, SerOptions::new()).map_err(to_rv)?;
        Ok(())
    };
    write(meta_data, bbox_specifics, "pickle", ser)
}

#[cfg(test)]
use {super::core::make_test_bbs, crate::util::DEFAULT_TMPDIR, serde_pickle::DeOptions};
#[cfg(test)]
fn make_data(extension: &str) -> (BboxToolData, MetaData) {
    let opened_folder = "xi".to_string();
    let test_export_folder = DEFAULT_TMPDIR.clone();
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
    let mut bbox_data = BboxToolData::new();
    bbox_data.push("x".to_string(), None);
    let mut bbs = make_test_bbs();
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    bbs.extend(bbs.clone());
    let annos = bbox_data.get_annos_mut("dummyfile");
    for bb in bbs {
        annos.add_bb(bb, 0);
    }
    (bbox_data, meta)
}
#[test]
fn test_json_export() -> RvResult<()> {
    let (bbox_data, meta) = make_data("json");
    let written_path = write_json(&meta, bbox_data.clone())?;
    let s = fs::read_to_string(written_path).map_err(to_rv)?;
    let read: BboxDataIo = serde_json::from_str(s.as_str()).map_err(to_rv)?;
    assert_eq!(read.opened_folder, meta.opened_folder.unwrap());
    assert_eq!(read.ssh_cfg, None);
    assert_eq!(read.bbox_data, bbox_data);
    Ok(())
}
#[test]
fn test_pickle_export() -> RvResult<()> {
    let (bbox_data, meta) = make_data("pickle");
    let written_path = write_pickle(&meta, bbox_data.clone())?;
    let f = File::open(written_path).map_err(to_rv)?;
    let read: BboxDataIo = serde_pickle::from_reader(f, DeOptions::new()).map_err(to_rv)?;
    assert_eq!(read.opened_folder, meta.opened_folder.unwrap());
    assert_eq!(read.ssh_cfg, None);
    assert_eq!(read.bbox_data, bbox_data);
    Ok(())
}
