use std::{fs, path::PathBuf, thread, time::Duration};

use rvlib::{control::Control, defer_file_removal, world::ToolsDataMap};

fn get_test_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data")
}

fn get_ctrl() -> Control {
    let mut ctrl = Control::default();
    let test_folder = get_test_folder();
    ctrl.cfg.usr.home_folder = Some(test_folder.to_str().unwrap().to_string());
    ctrl
}

fn prj_load(file: &str) -> ToolsDataMap {
    let mut ctrl = get_ctrl();
    let test_file_src = get_test_folder().join(get_test_folder().join(file));
    let test_file = get_test_folder().join("tmp-test.rvi");
    defer_file_removal!(&test_file);
    fs::copy(&test_file_src, &test_file).unwrap();
    let tdm = ctrl.load(test_file.clone()).unwrap();
    thread::sleep(Duration::from_millis(5));
    tdm
}

#[test]
fn test_prj_v3_2() {
    prj_load("rvprj_v3-2_test_dummy.json");
}

#[test]
fn test_prj_v3_3() {
    prj_load("rvprj_v3-3_test_dummy.rvi");
}

#[test]
fn test_prj_v4_0() {
    prj_load("rvprj_v4-0.json");
}
