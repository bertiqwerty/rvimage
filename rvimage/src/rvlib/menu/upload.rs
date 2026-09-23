use std::fs;
use std::path::Path;

use egui::{RichText, Ui};
use rvimage_domain::RvResult;

use crate::image_reader::SUPPORTED_EXTENSIONS;
use crate::menu::ui_util::text_edit_singleline_noselect;
use crate::{control::Control, file_util};

#[derive(Default)]
pub struct UploadCreateState {
    pub target_folder_buffer: String,
    pub folder_selection_options: Vec<String>,
    pub show_new_modal: bool,
}

pub fn upload(
    ui: &mut Ui,
    upload_create_state: &mut UploadCreateState,
    are_tools_active: &mut bool,
    ctrl: &mut Control,
) -> RvResult<()> {
    let mut result_upload = Ok(());
    egui::modal::Modal::new(egui::Id::new("upload+create+folder")).show(ui.ctx(), |ui| {
        egui::Resize::default()
            .default_height(120.0)
            .default_width(220.0)
            .show(ui, |ui| {
                ui.heading("Upload");
                if let Some(upload_progress) = ctrl.upload_progress() {
                    ui.add(
                        egui::ProgressBar::new(upload_progress).text(
                            RichText::new(format!(
                                "uploading images {:.2}%",
                                (upload_progress * 100.0)
                            ))
                            .monospace(),
                        ),
                    );
                    if ui.button("Cancel upload").clicked() {
                        ctrl.upload_terminate();
                    }
                } else {
                    if let Some(ps) = ctrl.paths_navigator.paths_selector()
                        && egui::CollapsingHeader::new("Select target folder")
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .max_height(300.0)
                                    .show(ui, |ui| {
                                        for opt in &upload_create_state.folder_selection_options {
                                            if ui.button(opt).clicked() {
                                                upload_create_state.target_folder_buffer =
                                                    opt.clone();
                                            }
                                        }
                                    });
                            })
                            .header_response
                            .clicked()
                    {
                        let file_paths = ps.filtered_abs_file_paths();
                        let mut folders: Vec<&str> = vec![];

                        for p in file_paths.iter().flat_map(|p| Path::new(p).parent()) {
                            if let Ok(p) = file_util::path_to_str(p)
                                && !folders.contains(&p)
                            {
                                folders.push(p);
                            }
                        }
                        folders.sort();
                        upload_create_state.folder_selection_options = folders
                            .into_iter()
                            .map(|s| {
                                if s.is_empty() {
                                    ".".to_string()
                                } else {
                                    s.to_owned()
                                }
                            })
                            .collect();
                    }
                    text_edit_singleline_noselect(
                        ui,
                        &mut upload_create_state.target_folder_buffer,
                        are_tools_active,
                    );
                    if ui.button("Upload folder's top level").clicked() {
                        let folder = rfd::FileDialog::new().pick_folder();
                        if let Some(folder) = folder
                            && let Ok(reader) = fs::read_dir(folder)
                        {
                            let src_files = reader
                                .flatten()
                                .map(|entry| entry.path())
                                .filter(|p| {
                                    SUPPORTED_EXTENSIONS.iter().any(|ext| {
                                        match file_util::osstr_to_str(p.extension()) {
                                            Ok(ext_) => !ext.is_empty() && ext_ == &ext[1..],
                                            _ => false,
                                        }
                                    })
                                })
                                .collect::<Vec<_>>();

                            result_upload =
                                ctrl.upload(&src_files, &upload_create_state.target_folder_buffer);
                        }
                    }
                    if ui.button("Upload a few files").clicked() {
                        let src_files = rfd::FileDialog::new().pick_files();
                        if let Some(src_files) = src_files {
                            result_upload =
                                ctrl.upload(&src_files, &upload_create_state.target_folder_buffer);
                        }
                    }
                    if ui.button("Close").clicked() {
                        upload_create_state.show_new_modal = false;
                    }
                }
            });
    });
    result_upload
}
