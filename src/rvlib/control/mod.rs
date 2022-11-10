use crate::{cfg::Cfg, reader::ReaderFromCfg, result::RvResult, threadpool::ThreadPool};

pub mod paths_navigator;
use paths_navigator::PathsNavigator;
#[derive(Clone, Debug)]
pub enum Info {
    Error(String),
    Warning(String),
    None,
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
