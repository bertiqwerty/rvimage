use egui::Ui;
use rvimage_domain::RvResult;
use serde::{de::DeserializeOwned, Serialize};
use tracing::info;

use super::{main::TextBuffers, ui_util::button_triggerable_number};
use crate::{
    control::{Control, SortParams, SortType},
    file_util::PathPair,
    paths_selector::PathsSelector,
    tools_data::{AnnotationsMap, BboxToolData, BrushToolData, InstanceAnnotate},
    world::ToolsDataMap,
};
#[derive(Default)]
pub(super) struct Stats {
    pub n_files_filtered_info: Option<String>,
    pub n_files_annotated_info: Option<String>,
}
pub fn delete_annotations<T>(
    annotations_map: &mut AnnotationsMap<T>,
    paths: &[&PathPair],
) -> RvResult<()>
where
    T: Clone + Serialize + DeserializeOwned,
{
    for p in paths {
        annotations_map.remove_pp(p);
    }
    Ok(())
}
pub fn propagate_instance_annotations<T>(
    annotations_map: &mut AnnotationsMap<T>,
    paths: &[&PathPair],
) -> RvResult<()>
where
    T: InstanceAnnotate,
{
    let prop_anno_shape = annotations_map.get_pp(paths[0]).cloned();
    if let Some((prop_anno, shape)) = prop_anno_shape {
        for p in paths {
            annotations_map.insert_pp(p, (prop_anno.clone(), shape));
        }
    }
    Ok(())
}
pub fn n_instance_annotated_images<T>(
    annotations_map: &AnnotationsMap<T>,
    paths: &[&PathPair],
) -> usize
where
    T: InstanceAnnotate,
{
    paths
        .iter()
        .filter(|p| {
            if let Some((anno, _)) = annotations_map.get_pp(p) {
                !anno.elts().is_empty()
            } else {
                false
            }
        })
        .count()
}
#[allow(clippy::too_many_arguments)]
pub fn labels_and_sorting(
    ui: &mut Ui,
    filename_sort_type: &mut SortParams,
    ctrl: &mut Control,
    tools_data_map: &mut ToolsDataMap,
    text_buffers: &mut TextBuffers,
    active_tool_name: Option<&str>,
    are_tools_active: &mut bool,
    stats: &mut Stats,
) -> RvResult<()> {
    if ui.button("Export filtered filelist").clicked() {
        let export_path = rfd::FileDialog::new()
            .add_filter("json", &["json"])
            .set_file_name("filtered_filelist.json")
            .save_file();
        if let Some(ep) = export_path {
            ctrl.paths_navigator
                .export_filtered_filelist(ep, &text_buffers.filter_string)?
        }
    }
    let mut clicked_nat = false;
    let mut clicked_alp = false;
    ui.horizontal(|ui| {
        ui.label("Sort");
        clicked_nat = ui
            .radio_value(&mut filename_sort_type.kind, SortType::Natural, "naturally")
            .clicked();
        clicked_alp = ui
            .radio_value(
                &mut filename_sort_type.kind,
                SortType::Alphabetical,
                "alphabetically",
            )
            .clicked();
    });
    #[derive(PartialEq)]
    enum TmpSortBy {
        Filename,
        RelativePath,
    }
    let mut tmp_sort_by = if filename_sort_type.sort_by_filename {
        TmpSortBy::Filename
    } else {
        TmpSortBy::RelativePath
    };
    let mut clicked_relative = false;
    let mut clicked_filename = false;
    ui.horizontal(|ui| {
        ui.label("Sort by");
        clicked_relative = ui
            .radio_value(&mut tmp_sort_by, TmpSortBy::RelativePath, "Relative path")
            .clicked();
        clicked_filename = ui
            .radio_value(&mut tmp_sort_by, TmpSortBy::Filename, "Filename")
            .clicked();
    });
    filename_sort_type.sort_by_filename = matches!(tmp_sort_by, TmpSortBy::Filename);
    if clicked_nat || clicked_alp || clicked_relative || clicked_filename {
        ctrl.reload(*filename_sort_type)?;
    }
    if let Some(info) = &stats.n_files_filtered_info {
        ui.label(egui::RichText::new(info).monospace());
    }
    if let Some(info) = &stats.n_files_annotated_info {
        ui.label(egui::RichText::new(info).monospace());
    }
    let get_file_info = |ps: &PathsSelector| {
        let n_files_filtered = ps.len_filtered();
        Some(format!("{n_files_filtered} files"))
    };
    let get_annotation_info = |ps: &PathsSelector| {
        let len_n_filtered = (ps.len_filtered().checked_ilog10().unwrap_or(0) + 1) as usize;
        let paths = &ps.filtered_file_paths();
        tools_data_map
            .iter()
            .flat_map(|(active_tool_name, data)| {
                let n = data.specifics.apply(
                    |d: &BboxToolData| Ok(n_instance_annotated_images(&d.annotations_map, paths)),
                    |d: &BrushToolData| Ok(n_instance_annotated_images(&d.annotations_map, paths)),
                );
                n.ok().map(|n| {
                    format!(
                        "{n:width1$} have {active_tool_name:width2$} annos",
                        width1 = len_n_filtered,
                        width2 = 5
                    )
                })
            })
            .reduce(|s1, s2| format!("{s1}\n{s2}"))
    };
    if let Some(ps) = ctrl.paths_navigator.paths_selector() {
        if stats.n_files_filtered_info.is_none() {
            stats.n_files_filtered_info = get_file_info(ps);
        }
        if stats.n_files_annotated_info.is_none() {
            stats.n_files_annotated_info = get_annotation_info(ps);
        }
        if ui.button("Re-compute Stats").clicked() {
            stats.n_files_filtered_info = get_file_info(ps);
            stats.n_files_annotated_info = get_annotation_info(ps);
        }
        if let Some(active_tool_name) = active_tool_name {
            egui::CollapsingHeader::new("Danger Zone").show(ui, |ui| {
                egui::Grid::new("bbox-label-prop-del-grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        let n_prop: Option<usize> = button_triggerable_number(
                            ui,
                            &mut text_buffers.label_propagation_buffer,
                            are_tools_active,
                            "propagate labels",
                            "number of following images to propagate label to",
                        );
                        ui.end_row();
                        let n_del: Option<usize> = button_triggerable_number(
                            ui,
                            &mut text_buffers.label_deletion_buffer,
                            are_tools_active,
                            "delete labels",
                            "number of following images to delete label from",
                        );

                        if let Some(selected_file_idx) =
                            ctrl.paths_navigator.file_label_selected_idx()
                        {
                            if let Some(n_prop) = n_prop {
                                let end = (selected_file_idx + n_prop).min(ps.len_filtered());
                                let range = selected_file_idx..end;
                                let paths = &ps.filtered_file_paths()[range];
                                if !paths.is_empty() {
                                    info!(
                                        "propagating {} labels from {}",
                                        paths.len(),
                                        paths[0].path_relative()
                                    );
                                    if let Some(data) = tools_data_map.get_mut(active_tool_name) {
                                        let _ = data.specifics.apply_mut(
                                            |d| {
                                                propagate_instance_annotations(
                                                    &mut d.annotations_map,
                                                    paths,
                                                )
                                            },
                                            |d| {
                                                propagate_instance_annotations(
                                                    &mut d.annotations_map,
                                                    paths,
                                                )
                                            },
                                        );
                                    }
                                }
                            }
                            if let Some(n_del) = n_del {
                                let end = (selected_file_idx + n_del).min(ps.len_filtered());
                                let range = selected_file_idx..end;
                                let paths = &ps.filtered_file_paths()[range];
                                if !paths.is_empty() {
                                    info!(
                                        "deleting {} labels from {}",
                                        paths.len(),
                                        paths[0].path_relative()
                                    );
                                    if let Some(data) = tools_data_map.get_mut(active_tool_name) {
                                        let _ = data.specifics.apply_mut(
                                            |d| delete_annotations(&mut d.annotations_map, paths),
                                            |d| delete_annotations(&mut d.annotations_map, paths),
                                        );
                                    }
                                }
                            }
                        }
                    });
            });
        }
    } else {
        stats.n_files_filtered_info = None;
        stats.n_files_annotated_info = None;
    }
    Ok(())
}
