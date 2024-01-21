use std::path::PathBuf;

use rvlib::{
    control::Control,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME, ROT90_NAME},
    world::ToolsDataMap,
};

fn get_test_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data")
}

fn get_ctrl() -> Control {
    let mut ctrl = Control::default();
    let test_folder = get_test_folder();
    ctrl.cfg.home_folder = Some(test_folder.to_str().unwrap().to_string());
    ctrl
}

fn prj_load(file: &str) -> ToolsDataMap {
    let mut ctrl = get_ctrl();
    ctrl.load(get_test_folder().join(file)).unwrap()
}

#[test]
fn test_prj_load_v2_3() {
    prj_load("rvprj_v2-3_test_dummy.json");
}

#[test]
fn test_prj_load_v2_4() {
    prj_load("rvprj_v2-4_test_dummy.json");
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
fn test_prj_v3_5() {
    let mut ctrl = get_ctrl();

    let tdm = ctrl
        .load(get_test_folder().join("rvprj_v3-5_test_dummy.json"))
        .unwrap();
    tdm.get(BBOX_NAME).unwrap();
    tdm.get(BRUSH_NAME).unwrap();
    tdm.get(ATTRIBUTES_NAME).unwrap();
    let tdm = ctrl
        .import(
            get_test_folder().join("rvprj_v3-5_test_dummy.json"),
            "C:\\images",
            "/images_elsewhere",
        )
        .unwrap();
    let bbox = tdm.get(BBOX_NAME).unwrap();
    let bbox = bbox.specifics.bbox().unwrap();
    for (p, _) in bbox.anno_iter() {
        println!("{}", p);
        assert!(p.starts_with("/images_elsewhere/somesub/folder"));
    }
    assert_eq!(bbox.label_info.labels()[0], "fg");
    let brush = tdm.get(BRUSH_NAME).unwrap();
    let brush = brush.specifics.brush().unwrap();
    for (p, _) in brush.anno_iter() {
        println!("{}", p);
        assert!(p.starts_with("/images_elsewhere/somesub/folder"));
    }
    assert_eq!(brush.label_info.labels()[0], "fg");
    let attributes = tdm.get(ATTRIBUTES_NAME).unwrap();
    let attributes = attributes.specifics.attributes().unwrap();
    for (p, _) in attributes.anno_iter() {
        println!("{}", p);
        assert!(p.starts_with("/images_elsewhere/somesub/folder"));
    }
    let rot90 = tdm.get(ROT90_NAME).unwrap();
    let rot90 = rot90.specifics.rot90().unwrap();
    for (p, _) in rot90.anno_iter() {
        println!("{}", p);
        assert!(p.starts_with("/images_elsewhere/somesub/folder"));
    }
}
