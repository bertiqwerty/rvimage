use egui::Ui;
use std::mem;

use crate::{
    cfg::WandManyCfg,
    menu::{
        main::WandManyMenuBuffers,
        params_menu::{
            add_buffer_sorted, add_parameter_menu, existing_params_menu, no_more_cols,
            parammap_from_strtypes,
        },
        ui_util::{
            process_number, removable_rows, slider, text_edit_multiline, text_edit_singleline,
        },
    },
    paths_selector::PathsSelector,
    wand_many::{WandManyData, WandManyMessage},
};

pub fn predict_button(
    ui: &mut Ui,
    data: &WandManyData,
    paths_selector: Option<&PathsSelector>,
) -> Option<WandManyMenuResult> {
    let mut to_submit = None;
    if data.is_wandmany_running && ui.button("Cancel running prediction").clicked() {
        to_submit = Some(WandManyMenuResult::Cancel);
    } else if !data.is_wandmany_running && ui.button("Predict").clicked() {
        to_submit = Some(
            predict(paths_selector, data)
                .map(WandManyMenuResult::Submit)
                .unwrap_or(WandManyMenuResult::Nothing),
        );
    }
    to_submit
}

pub fn predict(
    paths_selector: Option<&PathsSelector>,
    data: &WandManyData,
) -> Option<(Vec<String>, Vec<String>)> {
    let files = paths_selector.map(|ps| {
        ps.filtered_abs_file_paths()
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
    });
    if let Some(files) = files {
        let subfolders_to_exclude = data.subfolders_to_exclude.clone();
        Some((files.clone(), subfolders_to_exclude))
    } else {
        tracing::warn!("No files selected to submit to wand annotator");
        None
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum WandManyMenuResult {
    Submit((Vec<String>, Vec<String>)),
    #[default]
    Nothing,
    Cancel,
}

pub fn wand_many_menu(
    ui: &mut Ui,
    data: &mut WandManyData,
    cfg: &mut WandManyCfg,
    are_tools_active: &mut bool,
    buffers: &mut WandManyMenuBuffers,
    show_wandmany: &mut bool,
    paths_selector: Option<&PathsSelector>,
) -> WandManyMenuResult {
    let mut to_submit = WandManyMenuResult::Nothing;
    let mut assess_tmp = data
        .messages
        .iter()
        .last()
        .and_then(|msg| msg.success_assessment);
    egui::modal::Modal::new(egui::Id::new("prj-import-section")).show(ui.ctx(), |ui| {
        ui.heading("Apply wand to annotate filtered files");
        let len_msgs = data.messages.len();
        let mut idx_to_remove = None;
        egui::CollapsingHeader::new("Parameters").show(ui, |ui| {
            let mut new_param_name = mem::take(&mut data.new_param_name_buffer);
            let mut new_param_val = mem::take(&mut data.new_param_val_buffer);
            let add_param;
            (new_param_name, new_param_val, add_param) = add_parameter_menu(
                ui,
                new_param_name,
                new_param_val,
                data.param_map.keys(),
                are_tools_active,
            );
            if add_param {
                tracing::info!("Adding parameter {new_param_name}");
                add_buffer_sorted(
                    &data.param_map,
                    &new_param_name,
                    "".to_string(),
                    &mut data.param_value_buffers,
                );
                data.param_map.insert(new_param_name, new_param_val);
            } else {
                data.new_param_name_buffer = new_param_name;
                data.new_param_val_buffer = new_param_val;
            }
            if let Some(pmap) =
                parammap_from_strtypes(ui, &mut data.strtypes_buffer, are_tools_active)
            {
                tracing::info!("new parameter map {pmap:?}");
                data.param_value_buffers = vec!["".into(); pmap.len()];
                data.param_map = pmap;
            }
            let params = mem::take(&mut data.param_map);
            let value_buffers = mem::take(&mut data.param_value_buffers);
            let res =
                existing_params_menu(ui, params, are_tools_active, no_more_cols, value_buffers);
            res.apply(
                &mut data.param_map,
                &mut data.param_value_buffers,
                &data.new_param_name_buffer,
            );
        });
        ui.separator();
        egui::CollapsingHeader::new("Comments").show(ui, |ui| {
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    idx_to_remove = removable_rows(ui, len_msgs, |ui, idx| {
                        if let Some(msg) = data.messages.get(idx) {
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
                            if let Some(response) = &msg.response {
                                egui::CollapsingHeader::new("Response").id_salt(idx).show(
                                    ui,
                                    |ui| {
                                        ui.label(response);
                                    },
                                );
                                if idx < len_msgs.saturating_sub(1) {
                                    ui.label(
                                        msg.success_assessment
                                            .map(|a| format!("assessment {a}"))
                                            .unwrap_or("".to_string()),
                                    );
                                } else {
                                    let mut assess_checkbx = assess_tmp.is_some();

                                    if ui.checkbox(&mut assess_checkbx, "assess result").clicked() {
                                        if assess_checkbx {
                                            assess_tmp = Some(50);
                                        } else {
                                            assess_tmp = None;
                                        }
                                    }
                                    if let Some(assess) = assess_tmp.as_mut() {
                                        slider(
                                            ui,
                                            are_tools_active,
                                            assess,
                                            0..=100,
                                            "assess result",
                                        );
                                    }
                                }
                            }
                        }
                        ui.separator();
                    });
                });
            if let Some(idx) = idx_to_remove {
                data.messages.remove(idx);
            }
            if let Some(assess_last) = data.messages.iter_mut().last() {
                assess_last.success_assessment = assess_tmp;
            }

            text_edit_multiline(ui, &mut buffers.comment, are_tools_active);

            ui.horizontal(|ui| {
                if ui.button("Add comment").clicked() && !buffers.comment.trim().is_empty() {
                    data.messages.push(WandManyMessage::from_comment(mem::take(
                        &mut buffers.comment,
                    )));
                }
                if ui.button("Clear").clicked() {
                    data.messages.clear();
                }
            });
        });
        ui.separator();
        egui::CollapsingHeader::new("Folders to exclude").show(ui, |ui| {
            text_edit_singleline(ui, &mut buffers.exclfolder, are_tools_active);
            if ui.button("Add folder to exclude").clicked() && !buffers.exclfolder.trim().is_empty()
            {
                data.subfolders_to_exclude
                    .push(mem::take(&mut buffers.exclfolder))
            }

            let n_folders = data.subfolders_to_exclude.len();
            ui.separator();
            if n_folders > 0 {
                ui.label("Folders to exclude");
                let mut idx_remove = None;
                egui::Grid::new("label_grid").num_columns(2).show(ui, |ui| {
                    idx_remove = removable_rows(ui, n_folders, |ui, idx| {
                        if let Some(folder) = data.subfolders_to_exclude.get(idx) {
                            ui.label(folder);
                            ui.end_row();
                        }
                    });
                });
                if let Some(idx) = idx_remove {
                    data.subfolders_to_exclude.remove(idx);
                }
            }
        });
        ui.separator();
        egui::CollapsingHeader::new("Server settings").show(ui, |ui| {
            text_edit_singleline(ui, &mut cfg.url, are_tools_active).on_hover_text("url");
            let timeout_label = "timeout (s)";
            if buffers.timeout.is_empty() {
                buffers.timeout = cfg.timeout_s.to_string();
            }
            if let (has_changed, Some(timeout)) =
                process_number(ui, are_tools_active, timeout_label, &mut buffers.timeout)
                && has_changed
            {
                cfg.timeout_s = timeout;
            }
        });
        ui.separator();
        if let Some(to_submit_) = predict_button(ui, data, paths_selector) {
            to_submit = to_submit_;
        }
        ui.separator();
        if ui.button("Close").clicked() {
            *show_wandmany = false;
        }
    });
    to_submit
}
