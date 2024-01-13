use std::{fs, path::PathBuf, thread, time::Duration};

use rvlib::{
    cfg, defer_file_removal, tracing_setup::tracing_setup, Event, Events, KeyCode, MainEventLoop,
};

fn get_test_folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test_data")
}
#[test]
fn test_main() {
    tracing_setup();
    let cfg = cfg::get_default_cfg();
    let test_file_src = get_test_folder().join("rvprj_v3-3_test_dummy.rvi");
    let test_file = get_test_folder().join("tmp-test.rvi");
    fs::copy(&test_file_src, &test_file).unwrap();
    defer_file_removal!(&test_file);
    let mut main_loop = MainEventLoop::new(cfg, Some(test_file.clone()));
    let events = Events::default();
    egui::__run_test_ctx(|ctx| {
        main_loop.one_iteration(&events, &ctx).unwrap();
    });
    main_loop.load_prj(Some(test_file.clone())).unwrap();
    let file_info_before = fs::metadata(test_file.as_path())
        .unwrap()
        .modified()
        .unwrap();
    thread::sleep(Duration::from_millis(1));
    egui::__run_test_ctx(|ctx| {
        main_loop.one_iteration(&events, &ctx).unwrap();
    });
    let file_info_before_2 = fs::metadata(test_file.as_path())
        .unwrap()
        .modified()
        .unwrap();
    thread::sleep(Duration::from_millis(1));
    let events =
        Events::default().events(vec![Event::Held(KeyCode::Ctrl), Event::Pressed(KeyCode::S)]);
    egui::__run_test_ctx(|ctx| {
        main_loop.one_iteration(&events, &ctx).unwrap();
    });
    thread::sleep(Duration::from_millis(10));
    let file_info_after = fs::metadata(test_file.as_path())
        .unwrap()
        .modified()
        .unwrap();
    assert_ne!(file_info_before, file_info_after);
    assert_eq!(file_info_before, file_info_before_2);
}
