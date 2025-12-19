use std::path::{Path, PathBuf};

use crate::{
    cache::{FileCache, FileCacheArgs, NoCache},
    cfg::{Cache, Cfg, Connection},
    paths_selector::PathsSelector,
    types::AsyncResultImage,
};
use rvimage_domain::RvResult;
#[cfg(feature = "azure_blob")]
use {
    super::azure_blob_reader::{AzureConnectionData, ReadImageFromAzureBlob},
    rvimage_domain::rverr,
};

use super::{
    core::{CloneDummy, LoadImageForGui, Loader},
    local_reader::ReadImageFromPath,
    py_http_reader::ReadImageFromPyHttp,
    ssh_reader::ReadImageFromSsh,
};

pub struct ReaderFromCfg {
    cfg: Cfg,
    reader: Box<dyn LoadImageForGui + Send>,
}
impl ReaderFromCfg {
    pub fn cfg(&self) -> &Cfg {
        &self.cfg
    }

    pub fn from_cfg(cfg: Cfg) -> RvResult<Self> {
        let n_ssh_reconnections = cfg.ssh_cfg().n_reconnection_attempts();
        Ok(Self {
            reader: match (&cfg.prj.connection, &cfg.usr.cache) {
                (Connection::Local, Cache::FileCache) => {
                    let args = cfg.usr.file_cache_args.clone();
                    Box::new(Loader::<FileCache<ReadImageFromPath, _>, _>::new(
                        FileCacheArgs {
                            cfg_args: args,
                            reader_args: CloneDummy {},
                        },
                        0,
                    )?)
                }
                (Connection::Ssh, Cache::FileCache) => {
                    let args = cfg.usr.file_cache_args.clone();

                    Box::new(
                        Loader::<FileCache<ReadImageFromSsh, _>, FileCacheArgs<_>>::new(
                            FileCacheArgs {
                                cfg_args: args,
                                reader_args: cfg.ssh_cfg(),
                            },
                            n_ssh_reconnections,
                        )?,
                    )
                }
                (Connection::Local, Cache::NoCache) => {
                    Box::new(Loader::<NoCache<ReadImageFromPath, _>, _>::new(
                        CloneDummy {},
                        0,
                    )?)
                }
                (Connection::Ssh, Cache::NoCache) => {
                    Box::new(Loader::<NoCache<ReadImageFromSsh, _>, _>::new(
                        cfg.ssh_cfg(),
                        n_ssh_reconnections,
                    )?)
                }
                (Connection::PyHttp, Cache::FileCache) => {
                    let args = cfg.usr.file_cache_args.clone();

                    Box::new(
                        Loader::<FileCache<ReadImageFromPyHttp, _>, FileCacheArgs<_>>::new(
                            FileCacheArgs {
                                cfg_args: args,
                                reader_args: (),
                            },
                            n_ssh_reconnections,
                        )?,
                    )
                }
                (Connection::PyHttp, Cache::NoCache) => {
                    Box::new(Loader::<NoCache<ReadImageFromPyHttp, _>, _>::new((), 0)?)
                }
                #[cfg(feature = "azure_blob")]
                (Connection::AzureBlob, Cache::FileCache) => {
                    let cache_args = cfg.usr.file_cache_args.clone();
                    let azure_cfg_prj = cfg
                        .prj
                        .azure_blob
                        .as_ref()
                        .ok_or_else(|| rverr!("no azure cfg found"))?;
                    let connection_string_path =
                        PathBuf::from(&azure_cfg_prj.connection_string_path);
                    let container_name = azure_cfg_prj.container_name.clone();
                    let blob_list_timeout_s = azure_cfg_prj.blob_list_timeout_s;

                    Box::new(Loader::<
                        FileCache<ReadImageFromAzureBlob, _>,
                        FileCacheArgs<_>,
                    >::new(
                        FileCacheArgs {
                            cfg_args: cache_args,
                            reader_args: AzureConnectionData {
                                current_prj_path: cfg.current_prj_path().to_path_buf(),
                                connection_string_path,
                                container_name,
                                blob_list_timeout_s,
                            },
                        },
                        n_ssh_reconnections,
                    )?)
                }
                #[cfg(feature = "azure_blob")]
                _ => {
                    Err(rverr!(
                        "configuration option ({:?}, {:?}) not implemented",
                        cfg.prj.connection,
                        cfg.usr.cache
                    ))?;
                    // return a dummy such that the compiler is happy
                    Box::new(Loader::<NoCache<ReadImageFromPath, _>, _>::new(
                        CloneDummy {},
                        0,
                    )?)
                }
            },
            cfg,
        })
    }
}
impl LoadImageForGui for ReaderFromCfg {
    fn clear_cache(&mut self) -> RvResult<()> {
        self.reader.clear_cache()
    }
    fn toggle_clear_cache_on_close(&mut self) {
        self.reader.toggle_clear_cache_on_close();
    }
    fn read_image(
        &mut self,
        file_selected_idx: usize,
        abs_file_paths: &[&str],
    ) -> AsyncResultImage {
        self.reader.read_image(file_selected_idx, abs_file_paths)
    }
    fn read_cached_image(
        &self,
        file_selected_idx: usize,
        abs_file_paths: &[&str],
    ) -> AsyncResultImage {
        self.reader
            .read_cached_image(file_selected_idx, abs_file_paths)
    }

    fn open_folder(&self, abs_folder_path: &str, prj_path: &Path) -> RvResult<PathsSelector> {
        self.reader.open_folder(abs_folder_path, prj_path)
    }
    fn cache_size_in_mb(&mut self) -> f64 {
        self.reader.cache_size_in_mb()
    }
}
