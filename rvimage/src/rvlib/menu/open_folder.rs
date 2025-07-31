use egui::Response;

use crate::{cfg::Connection, control::Control, result::trace_ok_err};

use super::picklist;
use rvimage_domain::{RvError, RvResult};

pub fn pick_by_connection(ctrl: &mut Control, resp: &Response) -> RvResult<bool> {
    let mut cancel = false;
    let picked = match &ctrl.cfg.prj.connection {
        Connection::Local => {
            if resp.clicked() {
                let sf = rfd::FileDialog::new().pick_folder();
                if sf.is_none() {
                    cancel = true;
                }
                sf.as_ref()
                    .and_then(|sf| {
                        trace_ok_err(sf.to_str().ok_or_else(|| {
                            RvError::new("could not transfer path to unicode string")
                        }))
                    })
                    .map(|sf| sf.to_string())
            } else {
                None
            }
        }
        Connection::Ssh => {
            let picklist_res = picklist::pick(
                ctrl.cfg
                    .prj
                    .ssh
                    .remote_folder_paths
                    .iter()
                    .map(|s| s.as_str()),
                resp,
            );
            picklist_res
        }
        Connection::PyHttp => {
            let picklist_res = picklist::pick(
                ctrl.cfg
                    .prj
                    .py_http_reader_cfg
                    .as_ref()
                    .ok_or_else(|| RvError::new("no http reader cfg given in cfg"))?
                    .server_addresses
                    .iter()
                    .map(|s| s.as_str()),
                resp,
            );
            picklist_res.map(|folder| {
                let n_slashes = folder.chars().rev().take_while(|c| *c == '/').count();
                folder[0..folder.len() - n_slashes].to_string()
            })
        }
        #[cfg(feature = "azure_blob")]
        Connection::AzureBlob => {
            if resp.clicked() {
                let address = ctrl
                    .cfg
                    .prj
                    .azure_blob
                    .as_ref()
                    .ok_or_else(|| RvError::new("no azure blob cfg given in cfg"))?
                    .prefix
                    .clone();
                Some(address)
            } else {
                None
            }
        }
    };
    if let Some(new_folder) = picked {
        ctrl.open_relative_folder(new_folder)?;
        Ok(false)
    } else if cancel {
        Ok(false)
    } else {
        Ok(true)
    }
}
