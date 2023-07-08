mod core;
mod from_cfg;
mod local_reader;
mod py_http_reader;
mod ssh_reader;

#[cfg(feature = "azure_blob")]
mod azure_blob_reader;

pub use self::{core::LoadImageForGui, from_cfg::ReaderFromCfg};
