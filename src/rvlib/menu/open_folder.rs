use egui::Ui;

use crate::{
    cfg::Connection,
    control::Control,
    result::{RvError, RvResult},
};

use super::picklist;

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
                Some(
                    sf.to_str()
                        .ok_or_else(|| RvError::new("could not transfer path to unicode string"))?
                        .to_string(),
                )
            }
            Connection::Ssh => picklist::pick(
                ui,
                ctrl.cfg
                    .ssh_cfg
                    .remote_folder_paths
                    .iter()
                    .map(|s| s.as_str()),
                500.0,
                &resp,
            )
            .map(|s| s.to_string()),
            Connection::Http => Some(
                ctrl.cfg
                    .py_http_reader_cfg
                    .as_ref()
                    .ok_or_else(|| RvError::new("no http reader server address given in cfg"))?
                    .server_address
                    .clone(),
            ),
        };
        if let Some(new_folder) = picked {
            ctrl.open_folder(new_folder)?;
            Ok(false)
        } else {
            Ok(true)
        }
    } else {
        Ok(false)
    }
}
