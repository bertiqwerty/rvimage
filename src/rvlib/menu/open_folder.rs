use egui::Ui;

use crate::{
    cfg::Connection,
    control::Control,
    result::{trace_ok, RvError, RvResult},
};

use super::picklist::{self, PicklistResult};

pub fn button(ui: &mut Ui, ctrl: &mut Control, open_folder_popup_open: bool) -> RvResult<bool> {
    let resp = ui.button("Open Folder");
    if resp.clicked() {
        Ok(true)
    } else if open_folder_popup_open {
        let mut cancel = false;
        let picked = match &ctrl.cfg.connection {
            Connection::Local => {
                let sf = rfd::FileDialog::new().pick_folder();
                if sf.is_none() {
                    cancel = true;
                }
                sf.as_ref()
                    .and_then(|sf| {
                        trace_ok(sf.to_str().ok_or_else(|| {
                            RvError::new("could not transfer path to unicode string")
                        }))
                    })
                    .map(|sf| sf.to_string())
            }
            Connection::Ssh => {
                let picklist_res = picklist::pick(
                    ui,
                    ctrl.cfg
                        .ssh_cfg
                        .remote_folder_paths
                        .iter()
                        .map(|s| s.as_str()),
                    500.0,
                    &resp,
                    "ssh-open-popup",
                );
                match picklist_res {
                    Some(PicklistResult::Picked(folder)) => Some(folder.to_string()),
                    Some(PicklistResult::Cancel) => {
                        cancel = true;
                        None
                    }
                    _ => None,
                }
            }
            Connection::PyHttp => {
                let picklist_res = picklist::pick(
                    ui,
                    ctrl.cfg
                        .py_http_reader_cfg
                        .as_ref()
                        .ok_or_else(|| RvError::new("no http reader cfg given in cfg"))?
                        .server_addresses
                        .iter()
                        .map(|s| s.as_str()),
                    500.0,
                    &resp,
                    "pyhttp-open-popup",
                );
                match picklist_res {
                    Some(PicklistResult::Picked(folder)) => Some({
                        let n_slashes = folder.chars().rev().take_while(|c| *c == '/').count();
                        folder[0..folder.len() - n_slashes].to_string()
                    }),
                    Some(PicklistResult::Cancel) => {
                        cancel = true;
                        None
                    }
                    _ => None,
                }
            }
            #[cfg(feature = "azure_blob")]
            Connection::AzureBlob => {
                let address = ctrl
                    .cfg
                    .azure_blob_cfg
                    .as_ref()
                    .ok_or_else(|| RvError::new("no azure blob cfg given in cfg"))?
                    .prefix
                    .clone();
                Some(address)
            }
        };
        if let Some(new_folder) = picked {
            ctrl.open_folder(new_folder)?;
            Ok(false)
        } else if cancel {
            Ok(false)
        } else {
            Ok(true)
        }
    } else {
        Ok(false)
    }
}
