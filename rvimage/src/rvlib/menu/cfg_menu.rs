use std::{
    fs::{self, File},
    path::Path,
};

use egui::{Popup, Response, RichText, Ui, Visuals, Widget};
use rvimage_domain::to_rv;

use crate::{
    cfg::{self, AzureBlobCfgPrj, Cache, Cfg, Connection, get_cfg_tmppath, write_cfg_str},
    file_util::get_prj_name,
    menu::ui_util::text_edit_singleline,
    result::trace_ok_err,
};

fn azure_cfg_menu(
    ui: &mut Ui,
    azure_cfg: &mut AzureBlobCfgPrj,
    curprjpath: Option<&Path>,
    are_tools_active: &mut bool,
) {
    egui::Grid::new("azure-cfg-menu")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("Connection str path");
            text_edit_singleline(ui, &mut azure_cfg.connection_string_path, are_tools_active)
                .on_hover_text(azure_cfg.connection_string_path.clone());
            if ui.button("browse").clicked() {
                let csf = rfd::FileDialog::new().pick_file();
                if let Some(csf) = csf {
                    let relpath = curprjpath
                        .and_then(|cpp| csf.strip_prefix(cpp).map_err(to_rv).ok())
                        .and_then(|rp| rp.to_str());
                    if let Some(relpath) = relpath {
                        azure_cfg.connection_string_path = relpath.to_string();
                    } else if let Some(csf_s) = csf.to_str() {
                        azure_cfg.connection_string_path = csf_s.to_string();
                    }
                }
            }
            ui.end_row();
            ui.label("Blob container name");
            text_edit_singleline(ui, &mut azure_cfg.container_name, are_tools_active)
                .on_hover_text(azure_cfg.container_name.clone());
            ui.end_row();
            ui.label("Prefix/folder");
            text_edit_singleline(ui, &mut azure_cfg.prefix, are_tools_active)
                .on_hover_text(azure_cfg.prefix.clone());
        });
}

enum Close {
    Yes(bool),
    No,
}

fn settings_popup(
    ui: &mut Ui,
    cfg: &mut Cfg,
    are_tools_active: &mut bool,
    toggle_cache_clear_on_close: &mut bool,
) -> Close {
    let mut close = Close::No;
    ui.horizontal(|ui| {
        if ui.button("Open in Editor").clicked() {
            // to show the current config in an external editor, we need to save it first
            let tmppath = get_cfg_tmppath(cfg);
            tmppath
                .parent()
                .and_then(|p| fs::create_dir_all(p).ok())
                .or_else(|| {
                    tracing::error!("could not create directory for tmp file");
                    Some(())
                });
            trace_ok_err(File::create(&tmppath));
            let log_tmp = false;
            if let Err(e) =
                toml::to_string_pretty(&cfg).map(|s| write_cfg_str(&s, &tmppath, log_tmp))
            {
                tracing::error!("could not write config,\n{e:#?}");
                tracing::error!("{:?}", cfg);
            }
            tracing::info!("opening {tmppath:?}");
            if let Err(e) = edit::edit_file(&tmppath) {
                tracing::error!("{e:?}");
                tracing::error!("could not open editor. {:?}", edit::get_editor());
            }
            if let Some(cfg_) = trace_ok_err(cfg::read_cfg_gen::<Cfg>(&tmppath)) {
                tracing::info!("config updated with new settings");
                tracing::info!("{:?}", cfg_);
                *cfg = cfg_;
            }
            if let Err(e) = cfg.write() {
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
        let name = get_prj_name(cfg.current_prj_path(), None);
        ui.label("Project Name");
        ui.label(RichText::from(name).text_style(egui::TextStyle::Monospace))
            .on_hover_text(
                cfg.current_prj_path()
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
            );
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Style");
        if ui.visuals().dark_mode {
            if ui.button("Light").clicked() {
                cfg.usr.darkmode = Some(false);
                ui.ctx().set_visuals(Visuals::light());
            }
        } else if ui.button("Dark").clicked() {
            cfg.usr.darkmode = Some(true);
            ui.ctx().set_visuals(Visuals::dark());
        }
    });
    ui.separator();
    ui.checkbox(&mut cfg.usr.hide_thumbs, "Hide Thumbnails");
    ui.checkbox(&mut cfg.usr.thumb_attrs_view, "Show Image Attribute List");
    ui.separator();
    if ui
        .checkbox(
            &mut cfg.usr.file_cache_args.clear_on_close,
            "Clear cache on close",
        )
        .changed()
    {
        *toggle_cache_clear_on_close = true;
    }
    ui.separator();
    ui.horizontal(|ui| {
        let mut autosave = cfg.usr.n_autosaves.unwrap_or(0);
        ui.label("Autosave versions");
        ui.add(egui::Slider::new(&mut autosave, 0..=10));
        if autosave > 0 {
            cfg.usr.n_autosaves = Some(autosave);
        } else {
            cfg.usr.n_autosaves = None;
        }
    });
    ui.separator();
    ui.label("Connection");
    ui.radio_value(&mut cfg.prj.connection, Connection::Local, "Local");
    ui.radio_value(&mut cfg.prj.connection, Connection::Ssh, "Ssh");
    ui.radio_value(
        &mut cfg.prj.connection,
        Connection::PyHttp,
        "Http served by 'python -m http.server'",
    );
    #[cfg(feature = "azure_blob")]
    ui.radio_value(
        &mut cfg.prj.connection,
        Connection::AzureBlob,
        "Azure blob storage",
    );
    #[cfg(feature = "azure_blob")]
    if cfg.prj.connection == Connection::AzureBlob {
        let curprjpath = cfg.current_prj_path().parent().map(|cpp| cpp.to_path_buf());
        let azure_cfg = match &mut cfg.prj.azure_blob {
            Some(cfg) => cfg,
            None => {
                cfg.prj.azure_blob = Some(AzureBlobCfgPrj::default());
                cfg.prj.azure_blob.as_mut().unwrap()
            }
        };
        azure_cfg_menu(ui, azure_cfg, curprjpath.as_deref(), are_tools_active);
    }
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Cache");
        ui.radio_value(&mut cfg.usr.cache, Cache::FileCache, "File Cache");
        ui.radio_value(&mut cfg.usr.cache, Cache::NoCache, "No Cache");
    });
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("OK").clicked() {
            close = Close::Yes(true);
        }
        if ui.button("Cancel").clicked() {
            close = Close::Yes(false);
        }
    });
    close
}

pub struct CfgMenu<'a> {
    cfg: &'a mut Cfg,
    cfg_orig: Cfg,
    are_tools_active: &'a mut bool,
    toggle_clear_cache_on_close: &'a mut bool,
}
impl<'a> CfgMenu<'a> {
    pub fn new(
        cfg: &'a mut Cfg,
        are_tools_active: &'a mut bool,
        reload: &'a mut bool,
    ) -> CfgMenu<'a> {
        let cfg_orig = cfg.clone();
        Self {
            cfg,
            cfg_orig,
            are_tools_active,
            toggle_clear_cache_on_close: reload,
        }
    }
}
impl Widget for CfgMenu<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let edit_cfg_btn_resp = ui.button("Settings");
        Popup::menu(&edit_cfg_btn_resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                let close = settings_popup(
                    ui,
                    self.cfg,
                    self.are_tools_active,
                    self.toggle_clear_cache_on_close,
                );
                if let Close::Yes(save) = close {
                    if save {
                        if let Err(e) = self.cfg.write() {
                            tracing::error!("could not write config,\n{e:#?}");
                            tracing::error!("{:?}", self.cfg);
                        }
                    } else {
                        *self.cfg = self.cfg_orig.clone();
                    }
                    ui.close();
                }
            });
        edit_cfg_btn_resp
    }
}
