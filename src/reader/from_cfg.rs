use crate::{
    cache::{FileCache, FileCacheArgs, FileCacheCfgArgs, NoCache},
    cfg::{get_cfg, Cache, Cfg, Connection},
    result::{RvError, RvResult}, types::AsyncResultImage,
};

use super::{
    core::{CloneDummy, LoadImageForGui, Loader},
    local_reader::{FileDialogPicker, ReadImageFromPath},
    ssh_reader::{ReadImageFromSsh, SshConfigPicker},
};

fn unwrap_file_cache_args(args: Option<FileCacheCfgArgs>) -> RvResult<FileCacheCfgArgs> {
    args.ok_or_else(|| RvError::new("cfg with file cache needs file_cache_args"))
}

pub struct ReaderFromCfg {
    reader: Box<dyn LoadImageForGui>,
}
impl ReaderFromCfg {
    pub fn new() -> RvResult<Self> {
        let cfg = get_cfg()?;
        Self::from_cfg(cfg)
    }
    pub fn from_cfg(cfg: Cfg) -> RvResult<Self> {
        let n_ssh_reconnections = cfg.ssh_cfg.n_reconnection_attempts();
        let tmpdir = cfg.tmpdir()?.to_string();
        Ok(Self {
            reader: match (cfg.connection, cfg.cache) {
                (Connection::Local, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args)?;
                    Box::new(Loader::<
                        FileCache<ReadImageFromPath, _>,
                        FileDialogPicker,
                        _,
                    >::new(
                        FileCacheArgs {
                            cfg_args: args,
                            reader_args: CloneDummy {},
                            tmpdir,
                        },
                        0,
                    )?)
                }
                (Connection::Ssh, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args)?;

                    Box::new(Loader::<
                        FileCache<ReadImageFromSsh, _>,
                        SshConfigPicker,
                        FileCacheArgs<_>,
                    >::new(
                        FileCacheArgs {
                            cfg_args: args,
                            reader_args: cfg.ssh_cfg,
                            tmpdir,
                        },
                        n_ssh_reconnections,
                    )?)
                }
                (Connection::Local, Cache::NoCache) => Box::new(Loader::<
                    NoCache<ReadImageFromPath, _>,
                    FileDialogPicker,
                    _,
                >::new(
                    CloneDummy {}, 0
                )?),
                (Connection::Ssh, Cache::NoCache) => Box::new(Loader::<
                    NoCache<ReadImageFromSsh, _>,
                    SshConfigPicker,
                    _,
                >::new(
                    cfg.ssh_cfg, n_ssh_reconnections
                )?),
            },
        })
    }
}
impl LoadImageForGui for ReaderFromCfg {
    fn read_image(&mut self, file_selected_idx: usize) -> AsyncResultImage {
        self.reader.read_image(file_selected_idx)
    }
    fn open_folder(&mut self) -> RvResult<()> {
        self.reader.open_folder()
    }
    fn file_selected_idx(&self) -> Option<usize> {
        self.reader.file_selected_idx()
    }
    fn select_file(&mut self, idx: usize) {
        self.reader.select_file(idx)
    }
    fn list_file_labels(&self, filter_str: &str) -> RvResult<Vec<(usize, String)>> {
        self.reader.list_file_labels(filter_str)
    }
    fn folder_label(&self) -> RvResult<String> {
        self.reader.folder_label()
    }
    fn file_selected_label(&self) -> RvResult<String> {
        self.reader.file_selected_label()
    }
    fn file_selected_path(&self) -> RvResult<String> {
        self.reader.file_selected_path()
    }
}
