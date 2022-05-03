use ssh2::Session;

use crate::{
    cache::{FileCache, FileCacheArgs, FileCacheCfgArgs, NoCache},
    cfg::{get_cfg, Cache, Cfg, Connection},
    result::{AsyncResultImage, RvError, RvResult},
    ssh,
};

use super::{
    core::{LoadImageForGui, Loader, ReadImageFromPath},
    local_reader::FileDialogPicker,
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
        Ok(Self {
            reader: match (cfg.connection, cfg.cache) {
                (Connection::Local, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args)?;
                    Box::new(Loader::<
                        FileCache<ReadImageFromPath, ()>,
                        FileDialogPicker,
                        _,
                    >::new(FileCacheArgs {
                        cfg_args: args,
                        reader_args: (),
                    }))
                }
                (Connection::Ssh, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args)?;
                    let sess = ssh::auth(&cfg.ssh_cfg)?;
                    type LoaderType = Loader<
                        FileCache<ReadImageFromSsh, Session>,
                        SshConfigPicker,
                        FileCacheArgs<Session>,
                    >;
                    Box::new(LoaderType::new(FileCacheArgs {
                        cfg_args: args,
                        reader_args: sess,
                    }))
                }
                (Connection::Local, Cache::NoCache) => {
                    Box::new(Loader::<NoCache<ReadImageFromPath, ()>, FileDialogPicker, _>::new(()))
                }
                (Connection::Ssh, Cache::NoCache) => {
                    let sess = ssh::auth(&cfg.ssh_cfg)?;
                    type LoaderType =
                        Loader<NoCache<ReadImageFromSsh, Session>, SshConfigPicker, Session>;
                    Box::new(LoaderType::new(sess))
                }
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
}
