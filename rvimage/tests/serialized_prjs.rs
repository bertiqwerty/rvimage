#[cfg(test)]
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
#[cfg(test)]
use rvlib::{
    control::Control, defer_file_removal, tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME, ROT90_NAME}, tracing_setup::init_tracing_for_tests, world::ToolsDataMap
};

#[cfg(test)]
fn get_test_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data")
}

#[cfg(test)]
fn get_ctrl() -> Control {
    let mut ctrl = Control::default();
    let test_folder = get_test_folder();
    ctrl.cfg.usr.home_folder = Some(test_folder.to_str().unwrap().to_string());
    ctrl
}

#[cfg(test)]
fn tmp_copy(src: &Path) -> PathBuf {
    let tmp_file_stem = src.file_stem().unwrap().to_str().unwrap();
    let tmp_file = get_test_folder().join(format!("tmp-{tmp_file_stem}"));
    tracing::debug!("Copying {src:?} to {tmp_file:?}");
    fs::copy(src, &tmp_file).unwrap();
    tmp_file
}

#[cfg(test)]
fn prj_load(file: &str) -> ToolsDataMap {
    init_tracing_for_tests();
    let mut ctrl = get_ctrl();
    let test_file_src = get_test_folder().join(get_test_folder().join(file));
    let test_file = tmp_copy(&test_file_src);
    defer_file_removal!(&test_file);
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
    init_tracing_for_tests();
    fn test(src1: &Path, src2: &Path, reference: &Path) {
        let mut ctrl = get_ctrl();
        let src1 = tmp_copy(&src1);
        defer_file_removal!(&src1);
        let mut tdm_merged = ctrl.load(src1.clone()).unwrap();
        thread::sleep(Duration::from_millis(5));
        ctrl.import(src2.to_path_buf(), &mut tdm_merged).unwrap();
        thread::sleep(Duration::from_millis(5));

        let reference = tmp_copy(&reference);
        defer_file_removal!(&reference);
        let tdm_ref = ctrl.load(reference.clone()).unwrap();
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
                    assert_eq!(merged_annos.len(), ref_annos.len());
                    assert!(merged_annos.len() > 0);
                    for elt in merged_annos.elts() {
                        assert!(ref_annos.elts().contains(elt));
                    }
                    assert_eq!(merged_annos, ref_annos);
                    assert_eq!(merged_shape, ref_shape);
                }
            };
        }
        tst!(BBOX_NAME, bbox);
        tst!(BRUSH_NAME, brush);

        let rot90_merged = tdm_merged[ROT90_NAME].specifics.rot90().unwrap();
        let rot90_ref = tdm_ref[ROT90_NAME].specifics.rot90().unwrap();
        for (file, rot_merged) in rot90_merged.anno_iter() {
            assert_eq!(*rot_merged, rot90_ref.annotations_map()[file]);
        }
        let attr_merged = tdm_merged[ATTRIBUTES_NAME].specifics.attributes().unwrap();
        let attr_ref = tdm_ref[ATTRIBUTES_NAME].specifics.attributes().unwrap();
        for (file, (attr, _)) in attr_merged.anno_iter() {
            assert_eq!(*attr, attr_ref.attr_map(file).cloned().unwrap());
        }
    }
    let test_file_src_1 =
        get_test_folder().join(get_test_folder().join("import-test-src-flowerlabel.json"));
    let test_file_src_2 =
        get_test_folder().join(get_test_folder().join("import-test-src-treelabel.json"));
    let test_file_src_ref = get_test_folder().join(get_test_folder().join("import-test.json"));
    test(&test_file_src_1, &test_file_src_2, &test_file_src_ref);
    test(&test_file_src_ref, &test_file_src_ref, &test_file_src_ref);
}
