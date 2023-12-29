use std::path::PathBuf;

use rvlib::control::Control;

fn get_test_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data")
}

fn test_prj_load(file: &str) {
    let mut ctrl = Control::default();
    let test_folder = get_test_folder();
    ctrl.cfg.export_folder = Some(test_folder.to_str().unwrap().to_string());
    ctrl.load(file).unwrap();
}

#[test]
fn test_prj_load_v2_3() {
    test_prj_load("rvprj_v2-3_test_dummy.json");
}

#[test]
fn test_prj_load_v2_4() {
    test_prj_load("rvprj_v2-4_test_dummy.json");
}
