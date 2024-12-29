use crate::{cfg::get_log_folder, file_util::DEFAULT_HOMEDIR};
use backtrace::Backtrace;
use std::{cell::RefCell, io};
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    fmt::{writer::MakeWriterExt, Layer},
    prelude::*,
};

thread_local! {
    pub static BACKTRACE: RefCell<Option<Backtrace>> = const { RefCell::new(None) };
}
/// # Panics
/// In case tracing cannot be setup properly.
pub fn tracing_setup() -> WorkerGuard {
    let log_folder = get_log_folder(&DEFAULT_HOMEDIR);
    let file_appender = tracing_appender::rolling::daily(log_folder, "log");
    let (file_appender, guard_flush_file) = tracing_appender::non_blocking(file_appender);
    let file_appender = Layer::new()
        .with_writer(file_appender.with_max_level(Level::INFO))
        .with_line_number(true)
        .compact()
        .with_ansi(false)
        .with_file(true);
    #[cfg(not(feature = "print_debug"))]
    let stdout = Layer::new()
        .with_writer(io::stdout.with_max_level(Level::INFO))
        .with_file(true)
        .with_line_number(true);
    #[cfg(feature = "print_debug")]
    let stdout = Layer::new()
        .with_writer(io::stdout.with_max_level(Level::DEBUG))
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
    guard_flush_file
}

use std::sync::Once;
static INIT: Once = Once::new();

pub fn init_tracing_for_tests() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .init();
    });
}
