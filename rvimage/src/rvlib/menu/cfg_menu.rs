use std::fs::{self, File};

use egui::{Area, Frame, Id, Order, Response, RichText, Ui, Visuals, Widget};
use rvimage_domain::to_rv;

use crate::{
    cfg::{self, get_cfg_tmppath, write_cfg_str, Cache, Cfg, Connection},
    file_util::get_prj_name,
    menu::ui_util::text_edit_singleline,
    result::trace_ok_err,
};

pub struct CfgMenu<'a> {
    id: Id,
    cfg: &'a mut Cfg,
    cfg_orig: Cfg,
    are_tools_active: &'a mut bool,
}
impl<'a> CfgMenu<'a> {
    pub fn new(id: Id, cfg: &'a mut Cfg, are_tools_active: &'a mut bool) -> CfgMenu<'a> {
        let cfg_orig = cfg.clone();
        Self {
            id,
            cfg,
            cfg_orig,
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
                                // to show the current config in an external editor, we need to save it first
                                let tmppath = get_cfg_tmppath(self.cfg);
                                tmppath
                                    .parent()
                                    .and_then(|p| fs::create_dir_all(p).ok())
                                    .or_else(|| {
                                        tracing::error!("could not create directory for tmp file");
                                        Some(())
                                    });
                                trace_ok_err(File::create(&tmppath));
                                let log_tmp = false;
                                if let Err(e) = toml::to_string_pretty(&self.cfg)
                                    .map(|s| write_cfg_str(&s, &tmppath, log_tmp))
                                {
                                    tracing::error!("could not write config,\n{e:#?}");
                                    tracing::error!("{:?}", self.cfg);
                                }
                                tracing::info!("opening {tmppath:?}");
                                if let Err(e) = edit::edit_file(&tmppath) {
                                    tracing::error!("{e:?}");
                                    tracing::error!(
                                        "could not open editor. {:?}",
                                        edit::get_editor()
                                    );
                                }
                                if let Some(cfg) = trace_ok_err(cfg::read_cfg_gen::<Cfg>(&tmppath))
                                {
                                    *self.cfg = cfg;
                                }
                                if let Err(e) = cfg::write_cfg(self.cfg) {
                                    tracing::error!("could not save cfg {e:?}");
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
                                    self.cfg.usr.darkmode = Some(false);
                                    ui.ctx().set_visuals(Visuals::light());
                                }
                            } else if ui.button("Dark").clicked() {
                                self.cfg.usr.darkmode = Some(true);
                                ui.ctx().set_visuals(Visuals::dark());
                            }
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            let mut autosave = self.cfg.usr.n_autosaves.unwrap_or(0);
                            ui.label("Autosave versions");
                            ui.add(egui::Slider::new(&mut autosave, 0..=10));
                            if autosave > 0 {
                                self.cfg.usr.n_autosaves = Some(autosave);
                            } else {
                                self.cfg.usr.n_autosaves = None;
                            }
                        });
                        ui.separator();
                        ui.label("Connection");
                        ui.radio_value(&mut self.cfg.prj.connection, Connection::Local, "Local");
                        ui.radio_value(&mut self.cfg.prj.connection, Connection::Ssh, "Ssh");
                        ui.radio_value(
                            &mut self.cfg.prj.connection,
                            Connection::PyHttp,
                            "Http served by 'python -m http.server'",
                        );
                        #[cfg(feature = "azure_blob")]
                        ui.radio_value(
                            &mut self.cfg.prj.connection,
                            Connection::AzureBlob,
                            "Azure blob experimental",
                        );
                        if self.cfg.prj.connection == Connection::AzureBlob {
                            let curprjpath = self
                                .cfg
                                .current_prj_path()
                                .parent()
                                .map(|cpp| cpp.to_path_buf());
                            if let Some(azure_cfg) = &mut self.cfg.prj.azure_blob {
                                egui::Grid::new("azure-cfg-menu")
                                    .num_columns(2)
                                    .show(ui, |ui| {
                                        ui.label("Connection str path");
                                        text_edit_singleline(
                                            ui,
                                            &mut azure_cfg.connection_string_path,
                                            self.are_tools_active,
                                        )
                                        .on_hover_text(azure_cfg.connection_string_path.clone());
                                        if ui.button("browse").clicked() {
                                            let csf = rfd::FileDialog::new().pick_file();
                                            if let Some(csf) = csf {
                                                let relpath = curprjpath
                                                    .and_then(|cpp| {
                                                        csf.strip_prefix(cpp).map_err(to_rv).ok()
                                                    })
                                                    .and_then(|rp| rp.to_str());
                                                if let Some(relpath) = relpath {
                                                    azure_cfg.connection_string_path =
                                                        relpath.to_string();
                                                } else if let Some(csf_s) = csf.to_str() {
                                                    azure_cfg.connection_string_path =
                                                        csf_s.to_string();
                                                }
                                            }
                                        }
                                        ui.end_row();

                                        ui.label("Blob container name");
                                        text_edit_singleline(
                                            ui,
                                            &mut azure_cfg.container_name,
                                            self.are_tools_active,
                                        )
                                        .on_hover_text(azure_cfg.container_name.clone());
                                        ui.end_row();
                                        ui.label("Prefix/folder");
                                        text_edit_singleline(
                                            ui,
                                            &mut azure_cfg.prefix,
                                            self.are_tools_active,
                                        )
                                        .on_hover_text(azure_cfg.prefix.clone());
                                    });
                            }
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("Cache");
                            ui.radio_value(&mut self.cfg.usr.cache, Cache::FileCache, "File Cache");
                            ui.radio_value(&mut self.cfg.usr.cache, Cache::NoCache, "No Cache");
                        });
                        ui.separator();
                        ui.separator();
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
                if save {
                    if let Err(e) = cfg::write_cfg(self.cfg) {
                        tracing::error!("could not write config,\n{e:#?}");
                        tracing::error!("{:?}", self.cfg);
                    }
                } else {
                    *self.cfg = self.cfg_orig.clone();
                }
                ui.memory_mut(|m| m.toggle_popup(self.id));
            }
            if !edit_cfg_btn_resp.clicked() && area_response.clicked_elsewhere() {
                ui.memory_mut(|m| m.toggle_popup(self.id));
                *self.cfg = self.cfg_orig.clone();
            }
        }
        edit_cfg_btn_resp
    }
}
