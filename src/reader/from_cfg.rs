use crate::{
    cache::{file_cache::FileCache, NoCache},
    cfg::{get_cfg, Cache, Cfg, Connection},
    reader::{local_reader::LocalReader, scp_reader::ScpReader, ReadImageFiles},
    result::RvResult,
};

pub struct ReaderFromCfg {
    reader: Box<dyn ReadImageFiles>,
}
impl ReaderFromCfg {
    pub fn new() -> RvResult<Self> {
        let cfg = get_cfg()?;
        Ok(Self::from_cfg(cfg))
    }
    pub fn from_cfg(cfg: Cfg) -> Self {
        Self {
            reader: match (cfg.connection, cfg.cache) {
                (Connection::Local, Cache::FileCache) => Box::new(LocalReader::<FileCache>::new()),
                (Connection::Scp, Cache::FileCache) => Box::new(ScpReader::<FileCache>::new()),
                (Connection::Local, Cache::NoCache) => Box::new(LocalReader::<NoCache>::new()),
                (Connection::Scp, Cache::NoCache) => Box::new(ScpReader::<NoCache>::new()),
            },
        }
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
    ) -> RvResult<image::ImageBuffer<image::Rgb<u8>, Vec<u8>>> {
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
