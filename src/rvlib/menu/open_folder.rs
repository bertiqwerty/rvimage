use egui::{Id, Response, Ui};

use crate::{
    cfg::{Cfg, Connection},
    control::{paths_navigator::PathsNavigator, trigger_reader_creation, Info, OpenedFolder},
    paths_selector::PathsSelector,
    reader::ReaderFromCfg,
    result::{RvError, RvResult},
    threadpool::ThreadPool,
};

fn show_folder_list_popup(
    ui: &mut Ui,
    folders: &[String],
    popup_id: Id,
    below_respone: &Response,
) -> Option<usize> {
    ui.memory().open_popup(popup_id);
    let mut selected_idx = None;
    egui::popup_below_widget(ui, popup_id, below_respone, |ui| {
        ui.set_min_width(500.0);
        for (i, f) in folders.iter().enumerate() {
            if ui.button(f).clicked() {
                selected_idx = Some(i);
            }
        }
    });
    selected_idx
}

fn pick_folder_from_list(
    ui: &mut Ui,
    folder_list: &[String],
    response: &Response,
) -> RvResult<OpenedFolder> {
    let popup_id = ui.make_persistent_id("ssh-folder-popup");
    let idx = show_folder_list_popup(ui, folder_list, popup_id, response);
    match idx {
        Some(idx) => Ok(OpenedFolder::Some(folder_list[idx].clone())),
        None => Ok(OpenedFolder::PopupOpen),
    }
}

pub fn button(
    ui: &mut Ui,
    paths_navigator: &mut PathsNavigator,
    opened_folder: OpenedFolder,
    cfg: Cfg,
    last_open_folder_job_id: &mut Option<u128>,
    tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
) -> RvResult<OpenedFolder> {
    let resp = ui.button("open folder");
    if resp.clicked() {
        Ok(OpenedFolder::PopupOpen)
    } else {
        match opened_folder {
            OpenedFolder::PopupOpen => {
                let picked = match cfg.connection {
                    Connection::Local => {
                        let sf = rfd::FileDialog::new()
                            .pick_folder()
                            .ok_or_else(|| RvError::new("Could not pick folder."))?;
                        OpenedFolder::Some(
                            sf.to_str()
                                .ok_or_else(|| {
                                    RvError::new("could not transfer path to unicode string")
                                })?
                                .to_string(),
                        )
                    }
                    Connection::Ssh => {
                        pick_folder_from_list(ui, &cfg.ssh_cfg.remote_folder_paths, &resp)?
                    }
                };
                if let OpenedFolder::Some(_) = &picked {
                    (*paths_navigator, *last_open_folder_job_id) =
                        trigger_reader_creation(tp, cfg)?;
                }
                Ok(picked)
            }
            _ => Ok(opened_folder),
        }
    }
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
