use crate::{
    cache::{FileCache, FileCacheArgs, FileCacheCfgArgs, NoCache},
    cfg::{Cache, Cfg, Connection},
    paths_selector::PathsSelector,
    types::AsyncResultImage,
};
use rvimage_domain::{RvError, RvResult};
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

fn unwrap_file_cache_args(args: Option<FileCacheCfgArgs>) -> RvResult<FileCacheCfgArgs> {
    args.ok_or_else(|| RvError::new("cfg with file cache needs file_cache_args"))
}

pub struct ReaderFromCfg {
    cfg: Cfg,
    reader: Box<dyn LoadImageForGui + Send>,
}
impl ReaderFromCfg {
    pub fn cfg(&self) -> &Cfg {
        &self.cfg
    }

    pub fn from_cfg(cfg: Cfg) -> RvResult<Self> {
        let n_ssh_reconnections = cfg.ssh_cfg.n_reconnection_attempts();
        let tmpdir = format!("{}/{}", cfg.tmpdir()?, uuid::Uuid::new_v4());
        Ok(Self {
            reader: match (&cfg.connection, &cfg.cache) {
                (Connection::Local, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args.clone())?;
                    Box::new(Loader::<FileCache<ReadImageFromPath, _>, _>::new(
                        FileCacheArgs {
                            cfg_args: args,
                            reader_args: CloneDummy {},
                            tmpdir,
                        },
                        0,
                    )?)
                }
                (Connection::Ssh, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args.clone())?;

                    Box::new(
                        Loader::<FileCache<ReadImageFromSsh, _>, FileCacheArgs<_>>::new(
                            FileCacheArgs {
                                cfg_args: args,
                                reader_args: cfg.ssh_cfg.clone(),
                                tmpdir,
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
                        cfg.ssh_cfg.clone(),
                        n_ssh_reconnections,
                    )?)
                }
                (Connection::PyHttp, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args.clone())?;

                    Box::new(
                        Loader::<FileCache<ReadImageFromPyHttp, _>, FileCacheArgs<_>>::new(
                            FileCacheArgs {
                                cfg_args: args,
                                reader_args: (),
                                tmpdir,
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
                    let cache_args = unwrap_file_cache_args(cfg.file_cache_args.clone())?;
                    let azure_cfg = cfg
                        .azure_blob_cfg
                        .as_ref()
                        .ok_or_else(|| rverr!("no azure cfg found"))?;
                    let connection_string_path = azure_cfg.connection_string_path.clone();
                    let container_name = azure_cfg.container_name.clone();

                    Box::new(Loader::<
                        FileCache<ReadImageFromAzureBlob, _>,
                        FileCacheArgs<_>,
                    >::new(
                        FileCacheArgs {
                            cfg_args: cache_args,
                            reader_args: AzureConnectionData {
                                connection_string_path,
                                container_name,
                            },
                            tmpdir,
                        },
                        n_ssh_reconnections,
                    )?)
                }
                #[cfg(feature = "azure_blob")]
                _ => {
                    Err(rverr!(
                        "configuration option ({:?}, {:?}) not implemented",
                        cfg.connection,
                        cfg.cache
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
    fn read_image(
        &mut self,
        file_selected_idx: usize,
        file_paths: &[&str],
        reload: bool,
    ) -> AsyncResultImage {
        self.reader
            .read_image(file_selected_idx, file_paths, reload)
    }

    fn open_folder(&self, folder_path: &str) -> RvResult<PathsSelector> {
        self.reader.open_folder(folder_path)
    }
}
