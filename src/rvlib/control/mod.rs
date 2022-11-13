use crate::{
    cfg::Cfg, reader::ReaderFromCfg, result::RvResult, threadpool::ThreadPool,
    types::AsyncResultImage,
};

pub mod paths_navigator;
use crate::reader::LoadImageForGui;
use paths_navigator::PathsNavigator;

#[derive(Clone, Debug)]
pub enum Info {
    Error(String),
    Warning(String),
    None,
}
#[derive(Default)]
pub struct Control {
    pub reader: Option<ReaderFromCfg>,
    pub paths_navigator: PathsNavigator,
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

pub fn trigger_reader_creation(
    tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
    cfg: Cfg,
) -> RvResult<(PathsNavigator, Option<u128>)> {
    Ok((
        PathsNavigator::new(None),
        Some(tp.apply(Box::new(move || make_reader_from_cfg(cfg)))?),
    ))
}
