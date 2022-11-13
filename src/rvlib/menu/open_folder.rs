use egui::{Id, Response, Ui};

use crate::{
    cfg::Connection,
    control::Control,
    result::{RvError, RvResult},
};

type OpenedFolder = Option<String>;

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

fn pick_folder_from_list(ui: &mut Ui, folder_list: &[String], response: &Response) -> OpenedFolder {
    let popup_id = ui.make_persistent_id("ssh-folder-popup");
    let idx = show_folder_list_popup(ui, folder_list, popup_id, response);
    idx.map(|idx| folder_list[idx].clone())
}

pub fn button(ui: &mut Ui, ctrl: &mut Control, open_folder_popup_open: bool) -> RvResult<bool> {
    let resp = ui.button("open folder");
    if resp.clicked() {
        Ok(true)
    } else if open_folder_popup_open {
        let picked = match &ctrl.cfg.connection {
            Connection::Local => {
                let sf = rfd::FileDialog::new()
                    .pick_folder()
                    .ok_or_else(|| RvError::new("Could not pick folder."))?;
                OpenedFolder::Some(
                    sf.to_str()
                        .ok_or_else(|| RvError::new("could not transfer path to unicode string"))?
                        .to_string(),
                )
            }
            Connection::Ssh => {
                pick_folder_from_list(ui, &ctrl.cfg.ssh_cfg.remote_folder_paths, &resp)
            }
        };
        if let OpenedFolder::Some(new_folder) = picked {
            ctrl.open_folder(new_folder)?;
            Ok(false)
        } else {
            Ok(true)
        }
    } else {
        Ok(false)
    }
}
