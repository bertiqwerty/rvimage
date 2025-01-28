use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use crate::{
    control::Control, defer_file_removal, tracing_setup::init_tracing_for_tests, ToolsDataMap,
};

pub fn get_ctrl() -> Control {
    let mut ctrl = Control::default();
    ctrl.cfg.usr.home_folder = Some(ctrl.cfg.tmpdir().to_string());
    ctrl
}

pub fn tmp_copy(src: &Path) -> PathBuf {
    let tmp_file_stem = src.file_stem().unwrap().to_str().unwrap();
    let tmp_file = get_test_folder().join(format!("tmp-{tmp_file_stem}"));
    tracing::debug!("Copying {src:?} to {tmp_file:?}");
    fs::copy(src, &tmp_file).unwrap();
    tmp_file
}

pub fn get_test_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data")
}
pub fn prj_load(file: &str) -> ToolsDataMap {
    init_tracing_for_tests();
    let mut ctrl = get_ctrl();
    let test_file_src = get_test_folder().join(get_test_folder().join(file));
    let test_file = tmp_copy(&test_file_src);
    defer_file_removal!(&test_file);
    let mut tdm = ctrl.load(test_file.clone()).unwrap();
    let pre_tdm = tdm.clone();
    ctrl.import_annos(&test_file, &mut tdm).unwrap();
    thread::sleep(Duration::from_millis(5));
    assert_eq!(tdm, pre_tdm);
    tdm
}
