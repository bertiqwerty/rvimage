use crate::{
    cache::{FileCache, FileCacheArgs, NoCache},
    cfg::{get_cfg, Cache, Cfg, Connection},
    result::{RvError, RvResult, AsyncResultImage},
};

use super::{
    core::{ReadImageFiles, ReadImageFromPath, Reader},
    local_reader::FileDialogPicker,
    ssh_reader::{ReadImageFromSsh, SshConfigPicker},
};

fn unwrap_file_cache_args(args: Option<FileCacheArgs>) -> RvResult<FileCacheArgs> {
    args.ok_or_else(|| RvError::new("cfg with file cache needs file_cache_args"))
}

pub struct ReaderFromCfg {
    reader: Box<dyn ReadImageFiles>,
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
                    Box::new(
                        Reader::<FileCache<ReadImageFromPath>, FileDialogPicker, _>::new(args),
                    )
                }
                (Connection::Ssh, Cache::FileCache) => {
                    let args = unwrap_file_cache_args(cfg.file_cache_args)?;
                    Box::new(Reader::<FileCache<ReadImageFromSsh>, SshConfigPicker, _>::new(args))
                }
                (Connection::Local, Cache::NoCache) => {
                    Box::new(Reader::<NoCache<ReadImageFromPath>, FileDialogPicker, _>::new(()))
                }
                (Connection::Ssh, Cache::NoCache) => {
                    Box::new(Reader::<NoCache<ReadImageFromSsh>, SshConfigPicker, _>::new(()))
                }
            },
        })
    }
}
impl ReadImageFiles for ReaderFromCfg {
    fn next(&mut self) {
        self.reader.next();
    }
    fn prev(&mut self) {
        self.reader.prev();
    }
    fn read_image(
        &mut self,
        file_selected_idx: usize,
    ) -> AsyncResultImage {
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
    fn list_file_labels(&self) -> RvResult<Vec<String>> {
        self.reader.list_file_labels()
    }
    fn folder_label(&self) -> RvResult<String> {
        self.reader.folder_label()
    }
    fn file_selected_label(&self) -> RvResult<String> {
        self.reader.file_selected_label()
    }
}
