use std::path::{Path, PathBuf};

use crate::{
    cache::{FileCache, FileCacheArgs, FileCacheCfgArgs, NoCache},
    cfg::{get_default_cfg_usr, Cache, Cfg, Connection},
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

fn unwrap_file_cache_args(args: Option<FileCacheCfgArgs>) -> FileCacheCfgArgs {
    args.unwrap_or_else(|| {
        get_default_cfg_usr()
            .file_cache_args
            .expect("default usr cfg needs file_cache_args")
    })
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
        let n_ssh_reconnections = cfg.ssh_cfg().n_reconnection_attempts();
        let tmpdir = format!("{}/{}", cfg.tmpdir(), uuid::Uuid::new_v4());
        Ok(Self {
            reader: match (&cfg.prj.connection, &cfg.usr.cache) {
                (Connection::Local, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.usr.file_cache_args.clone());
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
                    let args = unwrap_file_cache_args(cfg.usr.file_cache_args.clone());

                    Box::new(
                        Loader::<FileCache<ReadImageFromSsh, _>, FileCacheArgs<_>>::new(
                            FileCacheArgs {
                                cfg_args: args,
                                reader_args: cfg.ssh_cfg(),
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
                        cfg.ssh_cfg(),
                        n_ssh_reconnections,
                    )?)
                }
                (Connection::PyHttp, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.usr.file_cache_args.clone());

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
                    let cache_args = unwrap_file_cache_args(cfg.usr.file_cache_args.clone());
                    let azure_cfg_prj = cfg
                        .prj
                        .azure_blob
                        .as_ref()
                        .ok_or_else(|| rverr!("no azure cfg found"))?;
                    let connection_string_path =
                        PathBuf::from(&azure_cfg_prj.connection_string_path);
                    let container_name = azure_cfg_prj.container_name.clone();

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
    fn read_image(
        &mut self,
        file_selected_idx: usize,
        abs_file_paths: &[&str],
        reload: bool,
    ) -> AsyncResultImage {
        self.reader
            .read_image(file_selected_idx, abs_file_paths, reload)
    }

    fn open_folder(&self, abs_folder_path: &str, prj_path: &Path) -> RvResult<PathsSelector> {
        self.reader.open_folder(abs_folder_path, prj_path)
    }
}
