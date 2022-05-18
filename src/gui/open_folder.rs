use egui::Ui;

use crate::{
    cfg::Cfg,
    reader::{LoadImageForGui, ReaderFromCfg},
    result::RvResult,
    threadpool::ThreadPool,
};

use super::Info;

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
    file_labels: &mut Vec<(usize, String)>,
    cfg: Cfg,
    last_open_folder_job_id: &mut Option<usize>,
    tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
) -> RvResult<()> {
    if ui.button("open folder").clicked() {
        *file_labels = vec![];

        *last_open_folder_job_id = Some(tp.apply(Box::new(move || make_reader_from_cfg(&cfg)))?);
    }
    Ok(())
}
pub fn check_if_connected(
    ui: &mut Ui,
    last_open_folder_job_id: &mut Option<usize>,
    mut reader: Option<ReaderFromCfg>,
    tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
) -> RvResult<(Option<ReaderFromCfg>, Vec<(usize, String)>, Info)> {
    let mut info_message = Info::None;
    let mut file_labels = vec![];
    if let Some(job_id) = last_open_folder_job_id {
        ui.label("connecting...");
        let tp_res = tp.result(*job_id);
        if let Some(reader_info_tmp) = tp_res {
            let mut new_reader = reader_info_tmp.0;
            info_message = reader_info_tmp.1;

            new_reader.open_folder()?;
            file_labels = new_reader.list_file_labels("")?;
            reader = Some(new_reader);
            *last_open_folder_job_id = None;
        }
    } else {
        ui.label(match &reader {
            Some(r) => r.folder_label()?,
            None => "".to_string(),
        });
    }
    Ok((reader, file_labels, info_message))
}
