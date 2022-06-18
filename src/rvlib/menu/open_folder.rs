use egui::{Id, Response, Ui};

use crate::{
    cfg::{Cfg, Connection},
    menu::{core::Info, paths_navigator::PathsNavigator},
    paths_selector::PathsSelector,
    reader::ReaderFromCfg,
    result::{RvError, RvResult},
    threadpool::ThreadPool,
};

pub enum OpenFolder {
    None,
    PopupOpen,
    Some(String),
}
fn show_ssh_folder_popup(
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

fn pick_ssh_folder(
    ui: &mut Ui,
    ssh_folders: &[String],
    response: &Response,
) -> RvResult<OpenFolder> {
    let popup_id = ui.make_persistent_id("ssh-folder-popup");
    let idx = show_ssh_folder_popup(ui, ssh_folders, popup_id, response);
    match idx {
        Some(idx) => Ok(OpenFolder::Some(ssh_folders[idx].clone())),
        None => Ok(OpenFolder::PopupOpen),
    }
}

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
    open_folder: OpenFolder,
    cfg: Cfg,
    last_open_folder_job_id: &mut Option<u128>,
    tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
) -> RvResult<OpenFolder> {
    let resp = ui.button("open folder");
    fn open_effects(
        tp: &mut ThreadPool<(ReaderFromCfg, Info)>,
        cfg: Cfg,
    ) -> RvResult<(PathsNavigator, Option<u128>)> {
        Ok((
            PathsNavigator::new(None),
            Some(tp.apply(Box::new(move || make_reader_from_cfg(&cfg)))?),
        ))
    }
    if resp.clicked() {
        match cfg.connection {
            Connection::Local => {
                let sf = rfd::FileDialog::new()
                    .pick_folder()
                    .ok_or_else(|| RvError::new("Could not pick folder."))?;
                (*paths_navigator, *last_open_folder_job_id) = open_effects(tp, cfg)?;
                Ok(OpenFolder::Some(
                    sf.to_str()
                        .ok_or_else(|| RvError::new("could not transfer path to unicode string"))?
                        .to_string(),
                ))
            }
            Connection::Ssh => Ok(OpenFolder::PopupOpen),
        }
    } else {
        match open_folder {
            OpenFolder::PopupOpen => {
                let picked = pick_ssh_folder(ui, &cfg.ssh_cfg.remote_folder_paths, &resp)?;
                if let OpenFolder::Some(_) = picked {
                    // this is when in the openfolder popup a folder has been selected
                    (*paths_navigator, *last_open_folder_job_id) = open_effects(tp, cfg)?;
                }
                Ok(picked)
            }
            _ => Ok(open_folder),
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
