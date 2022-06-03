use egui::Ui;

use crate::{
    cfg::Cfg,
    menu::{core::Info, paths_navigator::PathsNavigator},
    paths_selector::PathsSelector,
    reader::ReaderFromCfg,
    result::RvResult,
    threadpool::ThreadPool,
};

fn make_reader_from_cfg(cfg: &Cfg) -> (ReaderFromCfg, Info) {
    match ReaderFromCfg::from_cfg(cfg) {
        Ok(rfc) => (rfc, Info::None),
        Err(e) => (
            ReaderFromCfg::new().expect("default cfg broken"),
            Info::Warning(e.msg().to_string()),
        ),
    }
}
pub fn button(
    ui: &mut Ui,
    paths_navigator: &mut PathsNavigator,
    cfg: Cfg,
    last_open_folder_job_id: &mut Option<u128>,
    tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
) -> RvResult<()> {
    if ui.button("open folder").clicked() {
        *paths_navigator = PathsNavigator::new(None);

        *last_open_folder_job_id = Some(tp.apply(Box::new(move || make_reader_from_cfg(&cfg)))?);
    }
    Ok(())
}
pub fn check_if_connected(
    ui: &mut Ui,
    last_open_folder_job_id: &mut Option<u128>,
    paths_selector: &Option<PathsSelector>,
    tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
) -> RvResult<Option<(ReaderFromCfg, Info)>> {
    if let Some(job_id) = last_open_folder_job_id {
        ui.label("connecting...");
        let tp_res = tp.result(*job_id);
        if tp_res.is_some() {
            *last_open_folder_job_id = None;
        }
        Ok(tp_res)
    } else {
        ui.label(match paths_selector {
            Some(ps) => ps.folder_label(),
            None => "",
        });
        Ok(None)
    }
}
