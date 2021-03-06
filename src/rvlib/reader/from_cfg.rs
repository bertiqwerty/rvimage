use crate::{
    cache::{FileCache, FileCacheArgs, FileCacheCfgArgs, NoCache},
    cfg::{self, Cache, Cfg, Connection},
    paths_selector::PathsSelector,
    result::{RvError, RvResult},
    types::AsyncResultImage,
};

use super::{
    core::{CloneDummy, LoadImageForGui, Loader},
    local_reader::{LocalLister, ReadImageFromPath},
    ssh_reader::{ReadImageFromSsh, SshLister},
};

fn unwrap_file_cache_args(args: Option<FileCacheCfgArgs>) -> RvResult<FileCacheCfgArgs> {
    args.ok_or_else(|| RvError::new("cfg with file cache needs file_cache_args"))
}

pub struct ReaderFromCfg {
    reader: Box<dyn LoadImageForGui + Send>,
}
impl ReaderFromCfg {
    pub fn new() -> RvResult<Self> {
        let cfg = cfg::get_default_cfg();
        Self::from_cfg(&cfg)
    }
    pub fn from_cfg(cfg: &Cfg) -> RvResult<Self> {
        let n_ssh_reconnections = cfg.ssh_cfg.n_reconnection_attempts();
        let tmpdir = cfg.tmpdir()?.to_string();
        Ok(Self {
            reader: match (&cfg.connection, &cfg.cache) {
                (Connection::Local, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args.clone())?;
                    Box::new(
                        Loader::<FileCache<ReadImageFromPath, _>, LocalLister, _>::new(
                            FileCacheArgs {
                                cfg_args: args,
                                reader_args: CloneDummy {},
                                tmpdir,
                            },
                            0,
                        )?,
                    )
                }
                (Connection::Ssh, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args.clone())?;

                    Box::new(Loader::<
                        FileCache<ReadImageFromSsh, _>,
                        SshLister,
                        FileCacheArgs<_>,
                    >::new(
                        FileCacheArgs {
                            cfg_args: args,
                            reader_args: cfg.ssh_cfg.clone(),
                            tmpdir,
                        },
                        n_ssh_reconnections,
                    )?)
                }
                (Connection::Local, Cache::NoCache) => Box::new(Loader::<
                    NoCache<ReadImageFromPath, _>,
                    LocalLister,
                    _,
                >::new(
                    CloneDummy {}, 0
                )?),
                (Connection::Ssh, Cache::NoCache) => {
                    Box::new(Loader::<NoCache<ReadImageFromSsh, _>, SshLister, _>::new(
                        cfg.ssh_cfg.clone(),
                        n_ssh_reconnections,
                    )?)
                }
            },
        })
    }
}
impl LoadImageForGui for ReaderFromCfg {
    fn read_image(&mut self, file_selected_idx: usize, file_paths: &[String]) -> AsyncResultImage {
        self.reader.read_image(file_selected_idx, file_paths)
    }
    fn open_folder(&self, folder_path: &str) -> RvResult<PathsSelector> {
        self.reader.open_folder(folder_path)
    }
}
