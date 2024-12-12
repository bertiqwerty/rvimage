use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

use rvlib::{
    control::Control,
    defer_file_removal,
    tools::{BBOX_NAME, BRUSH_NAME},
    world::ToolsDataMap,
};

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

#[test]
fn test_import() {
    let mut ctrl = get_ctrl();
    let test_file_src_1 =
        get_test_folder().join(get_test_folder().join("import-test-src-flowerlabel.json"));
    let test_file_src_2 =
        get_test_folder().join(get_test_folder().join("import-test-src-treelabel.json"));
    let test_file_src_ref = get_test_folder().join(get_test_folder().join("import-test.json"));
    let mut tdm_merged = ctrl.load(test_file_src_1.clone()).unwrap();
    thread::sleep(Duration::from_millis(5));
    ctrl.import(test_file_src_2, &mut tdm_merged).unwrap();
    thread::sleep(Duration::from_millis(5));
    ctrl.save(Path::new("tmp_prj.json").to_path_buf(), &tdm_merged, false)
        .unwrap();

    let tdm_ref = ctrl.load(test_file_src_ref.clone()).unwrap();
    thread::sleep(Duration::from_millis(5));

    macro_rules! tst {
        ($name:expr, $acc:ident) => {
            let merged_map = tdm_merged[$name]
                .specifics
                .$acc()
                .unwrap()
                .annotations_map
                .clone();
            let ref_map = tdm_ref[$name]
                .specifics
                .$acc()
                .unwrap()
                .annotations_map
                .clone();
            for k in merged_map.keys() {
                let (merged_annos, merged_shape) = merged_map.get(k).unwrap();
                let (ref_annos, ref_shape) = ref_map.get(k).unwrap();
                for elt in merged_annos.elts() {
                    assert!(ref_annos.elts().contains(elt));
                }
                for elt in ref_annos.elts() {
                    assert!(merged_annos.elts().contains(elt));
                }
                assert_eq!(merged_annos, ref_annos);
                assert_eq!(merged_shape, ref_shape);
            }
            println!("{:?}", merged_map.keys().collect::<Vec<_>>());
            println!("{:?}", ref_map.keys().collect::<Vec<_>>());
        };
    }
    tst!(BBOX_NAME, bbox);
    tst!(BRUSH_NAME, brush);
}
