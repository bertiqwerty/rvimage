use std::os::windows::fs::MetadataExt;
#[cfg(test)]
use std::{fs, thread, time::Duration};

#[cfg(test)]
use rvlib::{
    defer_file_removal, get_test_folder, tracing_setup::init_tracing_for_tests, Event, Events,
    KeyCode, MainEventLoop,
};

#[test]
fn test_main() {
    init_tracing_for_tests();
    let test_file_src = get_test_folder().join("rvprj_v3-3_test_dummy.rvi");
    let test_file = get_test_folder().join("tmp-test.rvi");
    let test_file_src_2 = get_test_folder().join("rvprj_v4-0.json");
    let test_file_2 = get_test_folder().join("tmp-test-2.rvi");
    fs::copy(&test_file_src, &test_file).unwrap();
    fs::copy(&test_file_src_2, &test_file_2).unwrap();
    let size_before = fs::metadata(test_file.as_path()).unwrap().file_size();
    tracing::debug!("Size of {test_file:?} is {size_before} bytes");

    defer_file_removal!(&test_file);
    defer_file_removal!(&test_file_2);
    let mut main_loop = MainEventLoop::new(Some(test_file.clone()));
    let events = Events::default();
    egui::__run_test_ctx(|ctx| {
        main_loop.one_iteration(&events, None, ctx).unwrap();
    });
    main_loop.load_prj(Some(test_file.clone())).unwrap();
    thread::sleep(Duration::from_millis(1));
    egui::__run_test_ctx(|ctx| {
        main_loop.one_iteration(&events, None, ctx).unwrap();
    });
    thread::sleep(Duration::from_millis(1));
    main_loop.import_prj(&test_file_2).unwrap();
    egui::__run_test_ctx(|ctx| {
        main_loop.one_iteration(&events, None, ctx).unwrap();
    });
    thread::sleep(Duration::from_millis(1));
    let events =
        Events::default().events(vec![Event::Held(KeyCode::Ctrl), Event::Pressed(KeyCode::S)]);
    egui::__run_test_ctx(|ctx| {
        main_loop.one_iteration(&events, None, ctx).unwrap();
    });
    // lets wait for the file being written
    thread::sleep(Duration::from_millis(2000));
    let size_after = fs::metadata(test_file.as_path()).unwrap().file_size();
    tracing::debug!("Size of {test_file:?} is {size_after} bytes");
    assert!(size_before != size_after);
}
