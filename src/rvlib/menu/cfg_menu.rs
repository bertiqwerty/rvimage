use egui::{Area, Color32, Frame, Id, Order, Response, TextEdit, Ui, Widget};

use crate::{
    cfg::{self, Cache, Cfg, Connection, SshCfg},
    menu,
};

fn is_valid_ssh_cfg(s: &str) -> bool {
    toml::from_str::<SshCfg>(s).is_ok()
}
pub struct CfgMenu<'a> {
    id: Id,
    cfg: &'a mut Cfg,
    ssh_cfg_str: &'a mut String,
}
impl<'a> CfgMenu<'a> {
    pub fn new(id: Id, cfg: &'a mut Cfg, ssh_cfg_str: &'a mut String) -> CfgMenu<'a> {
        Self {
            id,
            cfg,
            ssh_cfg_str,
        }
    }
}
impl<'a> Widget for CfgMenu<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let edit_cfg_btn_resp = ui.button("settings");
        if edit_cfg_btn_resp.clicked() {
            ui.memory().toggle_popup(self.id);
        }
        enum Close {
            Yes(bool),
            No,
        }
        let mut close = Close::No;
        if ui.memory().is_popup_open(self.id) {
            let area = Area::new(self.id)
                .order(Order::Foreground)
                .default_pos(edit_cfg_btn_resp.rect.left_bottom());

            let area_response = area
                .show(ui.ctx(), |ui| {
                    Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("open in editor").clicked() {
                                match cfg::get_cfg_path() {
                                    Ok(p) => {
                                        if let Err(e) = edit::edit_file(p) {
                                            println!("{e:?}");
                                            println!(
                                                "could not open editor. {:?}",
                                                edit::get_editor()
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        println!("could not open config file. {e:?}");
                                    }
                                }
                                if let Ok(cfg) = cfg::get_cfg() {
                                    *self.cfg = cfg;
                                    *self.ssh_cfg_str =
                                        toml::to_string_pretty(&self.cfg.ssh_cfg).unwrap();
                                } else {
                                    println!("could not reload cfg from file");
                                }
                            }
                            if ui.button("OK").clicked() {
                                close = Close::Yes(true);
                            }
                            if ui.button("cancel").clicked() {
                                close = Close::Yes(false);
                            }
                        });
                        ui.separator();
                        ui.label("CONNECTION");
                        ui.radio_value(&mut self.cfg.connection, Connection::Local, "Local");
                        ui.radio_value(&mut self.cfg.connection, Connection::Ssh, "Ssh");
                        ui.radio_value(
                            &mut self.cfg.connection,
                            Connection::PyHttp,
                            "Http served by 'python -m http.server'",
                        );
                        #[cfg(feature = "azure_blob")]
                        ui.radio_value(
                            &mut self.cfg.connection,
                            Connection::AzureBlob,
                            "Azure blob experimental",
                        );
                        ui.separator();
                        ui.label("CACHE");
                        ui.radio_value(&mut self.cfg.cache, Cache::FileCache, "File cache");
                        ui.radio_value(&mut self.cfg.cache, Cache::NoCache, "No cache");
                        ui.separator();
                        ui.label("SSH CONNECTION PARAMETERS");
                        let clr = if is_valid_ssh_cfg(self.ssh_cfg_str) {
                            Color32::LIGHT_YELLOW
                        } else {
                            Color32::LIGHT_RED
                        };
                        ui.add(
                            TextEdit::multiline(self.ssh_cfg_str)
                                .desired_width(f32::INFINITY)
                                .code_editor()
                                .text_color(clr),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                close = Close::Yes(true);
                            }
                            if ui.button("cancel").clicked() {
                                close = Close::Yes(false);
                            }
                        })
                    });
                })
                .response;
            if let Close::Yes(save) = close {
                if save && is_valid_ssh_cfg(self.ssh_cfg_str) {
                    self.cfg.ssh_cfg = toml::from_str::<SshCfg>(self.ssh_cfg_str).unwrap();
                    if let Err(e) = cfg::write_cfg(self.cfg) {
                        println!("could not write config,\n{e:#?}");
                        println!("{:?}", self.cfg);
                    }
                } else {
                    let tmp = menu::core::get_cfg();
                    *self.cfg = tmp.0;
                }
                ui.memory().toggle_popup(self.id);
            }
            if !edit_cfg_btn_resp.clicked() && area_response.clicked_elsewhere() {
                ui.memory().toggle_popup(self.id);
                let tmp = menu::core::get_cfg();
                *self.cfg = tmp.0;
            }
        }
        edit_cfg_btn_resp
    }
}
