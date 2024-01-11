use egui::{Area, Color32, Frame, Id, Order, Response, RichText, TextEdit, Ui, Visuals, Widget};

use crate::{
    cfg::{self, Cache, Cfg, Connection, SshCfg},
    file_util::get_prj_name,
    menu::{self, ui_util},
};

fn is_valid_ssh_cfg(s: &str) -> bool {
    toml::from_str::<SshCfg>(s).is_ok()
}
pub struct CfgMenu<'a> {
    id: Id,
    cfg: &'a mut Cfg,
    ssh_cfg_str: &'a mut String,
    are_tools_active: &'a mut bool,
}
impl<'a> CfgMenu<'a> {
    pub fn new(
        id: Id,
        cfg: &'a mut Cfg,
        ssh_cfg_str: &'a mut String,
        are_tools_active: &'a mut bool,
    ) -> CfgMenu<'a> {
        Self {
            id,
            cfg,
            ssh_cfg_str,
            are_tools_active,
        }
    }
}
impl<'a> Widget for CfgMenu<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let edit_cfg_btn_resp = ui.button("Settings");
        if edit_cfg_btn_resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(self.id));
        }
        enum Close {
            Yes(bool),
            No,
        }
        let mut close = Close::No;
        if ui.memory(|m| m.is_popup_open(self.id)) {
            let area = Area::new(self.id)
                .order(Order::Foreground)
                .default_pos(edit_cfg_btn_resp.rect.left_bottom());

            let area_response = area
                .show(ui.ctx(), |ui| {
                    Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("Open in Editor").clicked() {
                                match cfg::get_cfg_path() {
                                    Ok(p) => {
                                        if let Err(e) = edit::edit_file(p) {
                                            tracing::error!("{e:?}");
                                            tracing::error!(
                                                "could not open editor. {:?}",
                                                edit::get_editor()
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("could not open config file. {e:?}");
                                    }
                                }
                                if let Ok(cfg) = cfg::get_cfg() {
                                    *self.cfg = cfg;
                                    *self.ssh_cfg_str =
                                        toml::to_string_pretty(&self.cfg.ssh_cfg).unwrap();
                                } else {
                                    tracing::error!("could not reload cfg from file");
                                }
                            }
                            if ui.button("OK").clicked() {
                                close = Close::Yes(true);
                            }
                            if ui.button("Cancel").clicked() {
                                close = Close::Yes(false);
                            }
                        });
                        ui.horizontal(|ui| {
                            let name = get_prj_name(self.cfg.current_prj_path(), None);
                            ui.label("Project Name");
                            ui.label(RichText::from(name).text_style(egui::TextStyle::Monospace));
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Style");
                            if ui.visuals().dark_mode {
                                if ui.button("Light").clicked() {
                                    self.cfg.darkmode = Some(false);
                                    ui.ctx().set_visuals(Visuals::light());
                                }
                            } else if ui.button("Dark").clicked() {
                                self.cfg.darkmode = Some(true);
                                ui.ctx().set_visuals(Visuals::dark());
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            let mut autosave = self.cfg.n_autosaves.unwrap_or(0);
                            ui.label("Autosave versions");
                            ui.add(egui::Slider::new(&mut autosave, 0..=10));
                            if autosave > 0 {
                                self.cfg.n_autosaves = Some(autosave);
                            } else {
                                self.cfg.n_autosaves = None;
                            }
                        });
                        ui.separator();
                        ui.label("Connection");
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
                        ui.horizontal(|ui| {
                            ui.label("Cache");
                            ui.radio_value(&mut self.cfg.cache, Cache::FileCache, "File Cache");
                            ui.radio_value(&mut self.cfg.cache, Cache::NoCache, "No Cache");
                        });
                        ui.separator();
                        ui.label("SSH Connection Parameters");
                        let multiline = |txt: &mut String| {
                            let clr = if is_valid_ssh_cfg(txt) {
                                Color32::LIGHT_BLUE
                            } else {
                                Color32::LIGHT_RED
                            };
                            ui.add(
                                TextEdit::multiline(txt)
                                    .desired_width(f32::INFINITY)
                                    .code_editor()
                                    .text_color(clr),
                            )
                        };
                        ui_util::text_edit_with_deactivated_tools(
                            self.ssh_cfg_str,
                            self.are_tools_active,
                            multiline,
                        );
                        ui.horizontal(|ui| {
                            if ui.button("OK").clicked() {
                                close = Close::Yes(true);
                            }
                            if ui.button("Cancel").clicked() {
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
                        tracing::error!("could not write config,\n{e:#?}");
                        tracing::error!("{:?}", self.cfg);
                    }
                } else {
                    let tmp = menu::main::get_cfg();
                    *self.cfg = tmp.0;
                }
                ui.memory_mut(|m| m.toggle_popup(self.id));
            }
            if !edit_cfg_btn_resp.clicked() && area_response.clicked_elsewhere() {
                ui.memory_mut(|m| m.toggle_popup(self.id));
                let tmp = menu::main::get_cfg();
                *self.cfg = tmp.0;
            }
        }
        edit_cfg_btn_resp
    }
}
