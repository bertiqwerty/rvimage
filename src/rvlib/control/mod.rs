use crate::format_rverr;
use crate::{
    cfg::Cfg, reader::ReaderFromCfg, result::RvResult, threadpool::ThreadPool,
    types::AsyncResultImage,
};

pub mod paths_navigator;
use crate::reader::LoadImageForGui;
use paths_navigator::PathsNavigator;

#[derive(Clone, Debug, Default)]
pub enum Info {
    Error(String),
    Warning(String),
    #[default]
    None,
}
#[derive(Default)]
pub struct Control {
    pub reader: Option<ReaderFromCfg>,
    pub info: Info,
    pub paths_navigator: PathsNavigator,
    pub opened_folder: Option<String>,
    tp: ThreadPool<(ReaderFromCfg, Info)>,
    last_open_folder_job_id: Option<u128>,
}
impl Control {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn reader(&self) -> Option<&ReaderFromCfg> {
        self.reader.as_ref()
    }
    pub fn read_image(&mut self, file_label_selected_idx: usize, reload: bool) -> AsyncResultImage {
        let wrapped_image = self.reader.as_mut().and_then(|r| {
            self.paths_navigator.paths_selector().as_ref().map(|ps| {
                let ffp = ps.filtered_file_paths();
                r.read_image(file_label_selected_idx, &ffp, reload)
            })
        });
        match wrapped_image {
            None => Ok(None),
            Some(x) => Ok(x?),
        }
    }
    fn make_reader(&mut self, cfg: Cfg) -> RvResult<()> {
        self.paths_navigator = PathsNavigator::new(None);
        self.last_open_folder_job_id =
            Some(self.tp.apply(Box::new(move || make_reader_from_cfg(cfg)))?);
        Ok(())
    }
    pub fn open_folder(&mut self, new_folder: String, cfg: Cfg) -> RvResult<()> {
        self.make_reader(cfg)?;
        self.opened_folder = Some(new_folder);
        Ok(())
    }
    pub fn check_if_connected(&mut self) -> RvResult<bool> {
        if let Some(job_id) = self.last_open_folder_job_id {
            let tp_res = self.tp.result(job_id);
            if let Some((reader, info)) = tp_res {
                self.last_open_folder_job_id = None;
                let opened_folder = self.opened_folder.as_deref().ok_or_else(|| {
                    format_rverr!("failed to open folder '{:?}'", self.opened_folder)
                })?;
                self.paths_navigator =
                    PathsNavigator::new(Some(reader.open_folder(opened_folder)?));
                self.reader = Some(reader);
                self.info = info;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(true)
        }
    }
    pub fn opened_folder_label(&self) -> Option<&str> {
        self.paths_navigator
            .paths_selector()
            .as_ref()
            .map(|ps| ps.folder_label())
    }
}
fn make_reader_from_cfg(cfg: Cfg) -> (ReaderFromCfg, Info) {
    match ReaderFromCfg::from_cfg(cfg) {
        Ok(rfc) => (rfc, Info::None),
        Err(e) => (
            ReaderFromCfg::new().expect("default cfg broken"),
            Info::Warning(e.msg().to_string()),
        ),
    }
}
