use crate::cfg::get_log_folder;
use backtrace::Backtrace;
use std::{cell::RefCell, io};
use tracing::Level;
use tracing_subscriber::{
    fmt::{writer::MakeWriterExt, Layer},
    prelude::*,
};
thread_local! {
    pub static BACKTRACE: RefCell<Option<Backtrace>> = RefCell::new(None);
}
pub fn tracing_setup() {
    let log_folder = get_log_folder().expect("no log folder");
    let file_appender = tracing_appender::rolling::daily(log_folder, "log");
    let (file_appender, _guard_file) = tracing_appender::non_blocking(file_appender);
    let file_appender = Layer::new()
        .with_writer(file_appender.with_max_level(Level::INFO))
        .with_line_number(true)
        .compact()
        .with_file(true);

    let stdout = Layer::new()
        .with_writer(io::stdout.with_max_level(Level::INFO))
        .with_file(true)
        .with_line_number(true);
    tracing_subscriber::registry()
        .with(file_appender)
        .with(stdout)
        .init();
    std::panic::set_hook(Box::new(|_| {
        let trace = Backtrace::new();
        BACKTRACE.with(move |b| b.borrow_mut().replace(trace));
    }));
}