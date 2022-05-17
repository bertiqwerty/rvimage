use egui::{Area, Color32, Frame, Id, Order, Response, TextEdit, Ui, Widget};

use crate::cfg::{self, Cache, Cfg, Connection, SshCfg};

fn save_on_click(resp: Response, cfg: &Cfg) -> String {
    if resp.clicked() {
        match cfg::write_cfg(cfg) {
            Ok(_) => "".to_string(),
            Err(e) => format!("{}", e),
        }
    } else {
        "".to_string()
    }
}
fn is_valid_ssh_cfg(s: &str) -> bool {
    toml::from_str::<SshCfg>(s).is_ok()
}
pub struct CfgGui<'a> {
    id: Id,
    cfg: &'a mut Cfg,
    ssh_cfg_str: &'a mut String,
}
impl<'a> CfgGui<'a> {
    pub fn new(id: Id, cfg: &'a mut Cfg, ssh_cfg_str: &'a mut String) -> CfgGui<'a> {
        Self {
            id,
            cfg,
            ssh_cfg_str,
        }
    }
}
impl<'a> Widget for CfgGui<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let edit_cfg_btn_resp = ui.button("settings");

        if edit_cfg_btn_resp.clicked() {
            ui.memory().toggle_popup(self.id);
        }
        if ui.memory().is_popup_open(self.id) {
            let area = Area::new(self.id)
                .order(Order::Foreground)
                .default_pos(edit_cfg_btn_resp.rect.left_bottom());

            let area_response = area
                .show(ui.ctx(), |ui| {
                    Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label("CONNECTION");
                        save_on_click(
                            ui.radio_value(&mut self.cfg.connection, Connection::Local, "Local"),
                            &self.cfg,
                        );
                        save_on_click(
                            ui.radio_value(&mut self.cfg.connection, Connection::Ssh, "Ssh"),
                            &self.cfg,
                        );
                        ui.separator();
                        ui.label("CACHE");
                        save_on_click(
                            ui.radio_value(&mut self.cfg.cache, Cache::FileCache, "File cache"),
                            &self.cfg,
                        );
                        save_on_click(
                            ui.radio_value(&mut self.cfg.cache, Cache::NoCache, "No cache"),
                            &self.cfg,
                        );
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
                    });
                })
                .response;
            if !edit_cfg_btn_resp.clicked() && area_response.clicked_elsewhere() {
                if is_valid_ssh_cfg(self.ssh_cfg_str) {
                    self.cfg.ssh_cfg = toml::from_str::<SshCfg>(self.ssh_cfg_str).unwrap();
                    cfg::write_cfg(self.cfg).unwrap();
                }
                ui.memory().toggle_popup(self.id);
            }
        }
        edit_cfg_btn_resp
    }
}
