use egui::Ui;
use std::mem;

use crate::{
    cfg::{WandManyCfg, WandManyMessage},
    menu::ui_util::{removable_rows, slider, text_edit_multiline, text_edit_singleline},
    paths_selector::PathsSelector,
};

pub fn wand_many_menu(
    ui: &mut Ui,
    cfg: &mut WandManyCfg,
    are_tools_active: &mut bool,
    comment_buffer: &mut String,
    exclfolder_buffer: &mut String,
    show_wandmany: &mut bool,
    paths_selector: Option<&PathsSelector>,
) -> Option<(Vec<String>, Vec<String>)> {
    let mut to_submit = None;
    let mut assess_tmp = cfg
        .messages
        .iter()
        .last()
        .and_then(|msg| msg.success_assessment);
    egui::modal::Modal::new(egui::Id::new("prj-import-section")).show(ui.ctx(), |ui| {
        ui.heading("Wand to annotate all filtered project images");
        let len_msgs = cfg.messages.len();
        let mut idx_to_remove = None;
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                idx_to_remove = removable_rows(ui, len_msgs, |ui, idx| {
                    if let Some(msg) = cfg.messages.get(idx) {
                        let mut job = egui::text::LayoutJob {
                            halign: egui::Align::RIGHT,
                            ..Default::default()
                        };
                        job.append(
                            &msg.comment,
                            0.0,
                            egui::TextFormat {
                                italics: true,
                                ..Default::default()
                            },
                        );
                        ui.label(job);
                        if idx < len_msgs.saturating_sub(1) {
                            ui.label(
                                msg.success_assessment
                                    .map(|a| format!("assessment {a}"))
                                    .unwrap_or("".to_string()),
                            );
                        } else if let Some(response) = &msg.response {
                            egui::CollapsingHeader::new("Response")
                                .id_salt(idx)
                                .show(ui, |ui| {
                                    ui.label(response);
                                });
                            let mut assess_checkbx = assess_tmp.is_some();
                            if ui.checkbox(&mut assess_checkbx, "assess result").clicked() {
                                if assess_checkbx {
                                    assess_tmp = Some(50u8);
                                } else {
                                    assess_tmp = None;
                                }
                            }
                            if let Some(assess) = assess_tmp.as_mut() {
                                slider(ui, are_tools_active, assess, 0..=100, "assess result");
                            }
                        }
                    }
                    ui.separator();
                });
            });
        if let Some(idx) = idx_to_remove {
            cfg.messages.remove(idx);
        }
        if let Some(assess_last) = cfg.messages.iter_mut().last() {
            assess_last.success_assessment = assess_tmp;
        }

        text_edit_multiline(ui, comment_buffer, are_tools_active);

        ui.horizontal(|ui| {
            if ui.button("Add comment").clicked() && !comment_buffer.trim().is_empty() {
                cfg.messages
                    .push(WandManyMessage::from_comment(mem::take(comment_buffer)));
            }
            if ui.button("Clear").clicked() {
                cfg.messages.clear();
            }
        });
        ui.separator();
        text_edit_singleline(ui, exclfolder_buffer, are_tools_active);
        if ui.button("Add folder to exclude").clicked() && !exclfolder_buffer.trim().is_empty() {
            cfg.subfolder_to_exclude.push(mem::take(exclfolder_buffer))
        }

        let n_folders = cfg.subfolder_to_exclude.len();
        ui.separator();
        if n_folders > 0 {
            ui.label("Folders to exclude");
            let mut idx_remove = None;
            egui::Grid::new("label_grid").num_columns(2).show(ui, |ui| {
                idx_remove = removable_rows(ui, n_folders, |ui, idx| {
                    if let Some(folder) = cfg.subfolder_to_exclude.get(idx) {
                        ui.label(folder);
                        ui.end_row();
                    }
                });
            });
            if let Some(idx) = idx_remove {
                cfg.subfolder_to_exclude.remove(idx);
            }
            ui.separator();
        }
        if ui.button("Submit").clicked() {
            *show_wandmany = false;
            let files = paths_selector.map(|ps| {
                ps.filtered_abs_file_paths()
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<String>>()
            });
            if let Some(files) = files {
                let subfolders_to_exclude = cfg.subfolder_to_exclude.clone();
                to_submit = Some((files.clone(), subfolders_to_exclude));
            } else {
                tracing::warn!("No files selected to submit to wand annotator");
            }
        }
        ui.separator();
        if ui.button("Close").clicked() {
            *show_wandmany = false;
        }
    });
    to_submit
}
