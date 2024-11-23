#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

mod cache;
pub mod cfg;
pub mod control;
mod drawme;
mod events;
pub mod file_util;
pub mod history;
pub mod httpserver;
mod image_reader;
pub mod image_util;
pub mod main_loop;
pub mod menu;
mod meta_data;
mod paths_selector;
pub mod result;
mod sort_params;
mod ssh;
mod threadpool;
pub mod tools;
mod tools_data;
pub mod tracing_setup;
mod types;
mod util;
pub mod view;
pub mod world;
pub use cfg::read_darkmode;
pub use drawme::{
    Annotation, BboxAnnotation, BrushAnnotation, Stroke, UpdateImage, UpdatePermAnnos,
    UpdateTmpAnno, UpdateView, UpdateZoomBox,
};
pub use events::{Event, Events, KeyCode, ZoomAmount};
pub use file_util::get_test_folder;
pub use main_loop::MainEventLoop;
pub use meta_data::MetaData;
pub use rvimage_domain::{
    bresenham_iter, color_with_intensity, BbI, BrushLine, GeoFig, Line, Polygon, ShapeI,
};
pub use tools_data::{
    coco_io::{read_coco, to_per_file_crowd, write_coco},
    InstanceAnnotate, Rot90ToolData,
};
