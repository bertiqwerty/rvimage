use chrono::{DateTime, Local};
use egui::{Area, Context, Frame, Id, Order, Response, RichText, Ui, Widget};
use egui_plot::{Corner, GridMark, Legend, MarkerShape, Plot, PlotPoint, PlotPoints, Points};
use rvimage_domain::{rverr, to_rv, RvResult};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::{HashMap, HashSet},
    f64, fs, iter, mem,
    ops::RangeInclusive,
    path::Path,
};

use crate::{
    autosave::{list_files, make_timespan, AUTOSAVE_KEEP_N_DAYS},
    control::{paths_navigator::PathsNavigator, Control},
    file_util::{self, PathPair},
    get_annos_from_tdm, get_labelinfo_from_tdm,
    paths_selector::PathsSelector,
    result::trace_ok_err,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
    tools_data::{
        attributes_data::{AttrMap, AttrVal},
        AnnotationsMap, AttributesToolData, ExportAsCoco, LabelInfo, ToolSpecifics, ToolsDataMap,
    },
    InstanceAnnotate,
};

use super::{
    main::TextBuffers,
    ui_util::{self, slider, ui_with_deactivated_tools_on_hover},
};

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

fn propagate_attributes(
    data: &mut AttributesToolData,
    paths: &[&PathPair],
    f: impl Fn(&AttrVal) -> AttrVal,
) -> RvResult<()> {
    let to_prop = mem::take(&mut data.to_propagate_attr_val);

    let prop_anno_shape = data.get_shape(paths[0].path_relative()).ok_or_else(|| {
        rverr!(
            "expecting annotations to be propagated exists for {:?}",
            paths[0]
        )
    })?;
    for (idx_to_prop, attr_val_to_prop) in &to_prop {
        for p in paths {
            data.set_attr_val(
                p.path_relative(),
                *idx_to_prop,
                f(attr_val_to_prop),
                prop_anno_shape,
            );
        }
    }
    data.to_propagate_attr_val = to_prop;
    Ok(())
}

fn propagate_annos_of_tool(
    tdm: &mut ToolsDataMap,
    tool_name: &'static str,
    paths: &[&PathPair],
) -> RvResult<()> {
    if let Some(data) = tdm.get_mut(tool_name) {
        println!("PROOOOOP");
        data.specifics.apply_mut(
            |d| propagate_instance_annotations(&mut d.annotations_map, paths),
            |d| propagate_instance_annotations(&mut d.annotations_map, paths),
            |d| propagate_attributes(d, paths, |attr_vals| attr_vals.clone()),
        )
    } else {
        Err(rverr!(
            "data of tool {tool_name} not found to propagate annotations"
        ))
    }
}
fn delete_subsequent_annos_of_tool(
    tdm: &mut ToolsDataMap,
    tool_name: &'static str,
    paths: &[&PathPair],
) -> RvResult<()> {
    if let Some(data) = tdm.get_mut(tool_name) {
        data.specifics.apply_mut(
            |d| delete_annotations(&mut d.annotations_map, paths),
            |d| delete_annotations(&mut d.annotations_map, paths),
            |d| propagate_attributes(d, paths, |attr_vals| attr_vals.clone().reset()),
        )
    } else {
        Err(rverr!(
            "data of tool {tool_name} not found to delete subsequent annotations"
        ))
    }
}

#[derive(Clone, Copy)]
enum Close {
    Yes,
    No,
}

fn fileinfo(path: &Path) -> RvResult<(String, String)> {
    let metadata = fs::metadata(path).map_err(to_rv)?;
    let n_bytes = metadata.len();
    let mb = n_bytes as f64 / (1024.0f64).powi(2);
    let mb = format!("{mb:0.3}mb");

    let modified = metadata.modified().map_err(to_rv)?;
    let datetime: DateTime<Local> = modified.into();
    let datetime = datetime.format("%b %d %Y - %H:%M:%S").to_string();
    Ok((mb, datetime))
}

struct FolderParams {
    max_n_folders: usize,
    parents_depth: u8,
}

fn ancestor(path: &String, depth: u8) -> &Path {
    Path::new(path)
        .ancestors()
        .nth(depth.into())
        .unwrap_or(Path::new(""))
}

#[derive(Clone, Copy, Default)]
pub enum FilterRelation {
    // files that are contained in the list of filtered files
    #[default]
    Available,
    // files that are NOT contained the list of filtered files
    Missing,
}
impl FilterRelation {
    fn apply<'a>(
        &'a self,
        mut filtered_filepaths: impl Iterator<Item = &'a &'a PathPair>,
        path_tdm_key: &'a str,
    ) -> bool {
        let is_key_in_filtered_paths =
            filtered_filepaths.any(|fp| fp.path_relative() == path_tdm_key);
        match self {
            Self::Available => is_key_in_filtered_paths,
            Self::Missing => !is_key_in_filtered_paths,
        }
    }
    fn select<T>(&self, option_available: T, option_missing: T) -> T {
        match self {
            Self::Available => option_available,
            Self::Missing => option_missing,
        }
    }
}

fn iter_attributes_of_files<'a>(
    atd: &'a AttributesToolData,
    filepaths: &'a [(usize, &PathPair)],
) -> impl Iterator<Item = (usize, &'a AttrMap)> + 'a {
    atd.anno_iter()
        .filter_map(move |(anno_key_filename, (attrmap, _))| {
            if let Some((idx, _)) = filepaths
                .iter()
                .find(|(_, fp)| fp.path_relative() == anno_key_filename)
            {
                Some((*idx, attrmap))
            } else {
                None
            }
        })
}
/// Returns an iterator over file idx, filename, toolname, number of annotations in file
fn iter_files_of_instance_tool<'a, T>(
    tdm: &'a ToolsDataMap,
    filepaths: &'a [(usize, &PathPair)],
    tool_name: &'static str,
    unwrap_specifics: impl Fn(&ToolSpecifics) -> RvResult<&AnnotationsMap<T>>,
    filter_relation: FilterRelation,
) -> RvResult<impl Iterator<Item = (Option<usize>, &'a str, &'static str, usize)> + 'a>
where
    T: InstanceAnnotate + 'a,
{
    if tdm.contains_key(tool_name) {
        let datamap = unwrap_specifics(&tdm[tool_name].specifics)?;

        let iter_available =
            filepaths
                .iter()
                .filter_map(move |(idx, filepath)| match filter_relation {
                    FilterRelation::Available => {
                        let annos = datamap.get(filepath.path_relative());
                        annos.map(|(annos, _)| {
                            (Some(*idx), filepath.path_relative(), tool_name, annos.len())
                        })
                    }
                    FilterRelation::Missing => None,
                });
        let iter_missing = datamap
            .iter()
            .filter(move |(k, _)| {
                matches!(filter_relation, FilterRelation::Missing)
                    && filter_relation.apply(filepaths.iter().map(|(_, fp)| fp), k)
            })
            .map(move |(k, (annos, _))| (None, k.as_str(), tool_name, annos.len()));
        Ok(iter_available.chain(iter_missing))
    } else {
        Err(rverr!("Tool {tool_name} has no data"))
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
pub struct ToolChoice {
    pub brush: bool,
    pub bbox: bool,
    pub attributes: bool,
}

impl ToolChoice {
    fn ui(&mut self, ui: &mut Ui, skip_attributes: bool) {
        ui.label("Select tool who's annotations you are interested in");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.bbox, BBOX_NAME);
            ui.checkbox(&mut self.brush, BRUSH_NAME);
            if !skip_attributes {
                ui.checkbox(&mut self.attributes, ATTRIBUTES_NAME)
                    .on_hover_text("only for propagation");
            }
        });
    }
    fn run_mut(
        &self,
        ui: &mut Ui,
        tdm: &mut ToolsDataMap,
        mut f_bbox: impl FnMut(&mut Ui, &mut ToolsDataMap) -> RvResult<()>,
        mut f_brush: impl FnMut(&mut Ui, &mut ToolsDataMap) -> RvResult<()>,
        mut f_attr: impl FnMut(&mut Ui, &mut ToolsDataMap) -> RvResult<()>,
    ) -> RvResult<()> {
        if self.bbox {
            f_bbox(ui, tdm)?;
        }
        if self.brush {
            f_brush(ui, tdm)?;
        }
        if self.attributes {
            f_attr(ui, tdm)?;
        }
        Ok(())
    }
    fn run<'a>(
        tool_name: &'static str,
        tdm: &'a ToolsDataMap,
        mut f_bbox: impl FnMut(&'a ToolsDataMap) -> RvResult<()>,
        mut f_brush: impl FnMut(&'a ToolsDataMap) -> RvResult<()>,
    ) -> RvResult<()> {
        match tool_name {
            BBOX_NAME => f_bbox(tdm),
            BRUSH_NAME => f_brush(tdm),
            _ => Err(rverr!("cannot run. unknown tool {tool_name}")),
        }
    }

    fn is_some(&self, skip_attributes: bool) -> bool {
        self.bbox || self.brush || (!skip_attributes && self.attributes)
    }
}

type ElementOfInstanceToolIterator<'a> = (Option<usize>, &'a str, &'static str, usize);
/// return a vector with filename, tool name, and number of annotations per file
fn get_all_files<'a>(
    tdm: &'a ToolsDataMap,
    filepaths: &'a [(usize, &PathPair)],
    absent_file_tool_choice: ToolChoice,
    filter_relation: FilterRelation,
) -> RvResult<Vec<ElementOfInstanceToolIterator<'a>>> {
    let mut all_absent_files = vec![];
    if absent_file_tool_choice.bbox {
        let mut all_absent_files_bbox = iter_files_of_instance_tool(
            tdm,
            filepaths,
            BBOX_NAME,
            |ts| ts.bbox().map(|d| &d.annotations_map),
            filter_relation,
        )?
        .collect::<Vec<_>>();
        all_absent_files.append(&mut all_absent_files_bbox);
    }
    if absent_file_tool_choice.brush {
        let mut all_absent_files_brush = iter_files_of_instance_tool(
            tdm,
            filepaths,
            BRUSH_NAME,
            |ts| ts.brush().map(|d| &d.annotations_map),
            filter_relation,
        )?
        .collect::<Vec<_>>();
        all_absent_files.append(&mut all_absent_files_brush);
    }
    Ok(all_absent_files)
}

fn tdm_instance_annos<T>(
    name: &str,
    tdm: &mut ToolsDataMap,
    ui: &mut Ui,
    folder_params: FolderParams,
    unwrap_specifics: impl Fn(&ToolSpecifics) -> RvResult<&AnnotationsMap<T>>,
    unwrap_specifics_mut: impl Fn(&mut ToolSpecifics) -> RvResult<&mut AnnotationsMap<T>>,
) -> RvResult<()>
where
    T: InstanceAnnotate,
{
    let FolderParams {
        max_n_folders,
        parents_depth,
    } = folder_params;
    if tdm.contains_key(name) {
        let annos = unwrap_specifics(&tdm[name].specifics)?;
        let mut n_annos_allfolders = 0;
        let parents_set = annos
            .iter()
            .map(|(k, (annos, _))| {
                n_annos_allfolders += annos.len();
                ancestor(k, parents_depth).to_path_buf()
            })
            .collect::<HashSet<_>>();
        let mut parents = parents_set.into_iter().collect::<Vec<_>>();
        parents.sort();
        let annos_map_mut = unwrap_specifics_mut(&mut tdm.get_mut(name).unwrap().specifics)?;

        ui.label(format!(
            "There are {n_annos_allfolders} {}-annotations{}.",
            name,
            if n_annos_allfolders > 0 {
                " of images in the following folders"
            } else {
                ""
            }
        ));
        egui::Grid::new("annotations-menu-grid").show(ui, |ui| {
            for p in &parents[0..max_n_folders.min(parents.len())] {
                let p_label = egui::RichText::new(p.to_str().unwrap_or("")).monospace();
                let n_annos_of_subfolders = egui::RichText::new(format!(
                    "{}",
                    annos_map_mut
                        .iter()
                        .filter(|(k, _)| ancestor(k, parents_depth) == p)
                        .map(|(_, (anno_map, _))| anno_map.len())
                        .sum::<usize>()
                ))
                .monospace();
                if ui
                    .button("x")
                    .on_hover_text("double-click to delete all annotations in this folder")
                    .double_clicked()
                {
                    let to_del = annos_map_mut
                        .keys()
                        .filter(|k| ancestor(k, parents_depth) == p)
                        .map(|k| k.to_string())
                        .collect::<Vec<_>>();
                    for k in to_del {
                        annos_map_mut.remove(&k);
                    }
                }
                ui.label(n_annos_of_subfolders);
                ui.label(p_label);

                ui.end_row();
            }
            if parents.len() > max_n_folders {
                ui.label(" ");
                ui.label(egui::RichText::new("...").monospace());
                ui.end_row();
            }
        });
    }
    Ok(())
}

#[derive(Default)]
pub struct AnnotationsParams {
    pub tool_choice_delprop: ToolChoice,
    pub tool_choice_stats: ToolChoice,
    pub tool_choice_plot: ToolChoice,
    pub parents_depth: u8,
    pub text_buffers: TextBuffers,
    pub filter_relation_deletion: FilterRelation,
    pub stats_result: Option<Vec<AnnoStatsRecord>>,
    pub selected_attributes_for_plot: HashMap<String, bool>,
    pub attribute_plots: HashMap<String, Vec<PlotPoint>>,
}

fn filter_relations_menu(
    heading: &'static str,
    ui: &mut Ui,
    filter_relation: FilterRelation,
) -> FilterRelation {
    ui.heading(heading);
    let mut is_missing = matches!(filter_relation, FilterRelation::Missing);
    ui.checkbox(&mut is_missing, "Delete annotations of missing files");
    if is_missing {
        FilterRelation::Missing
    } else {
        FilterRelation::Available
    }
}

fn annotations(
    ui: &mut Ui,
    tdm: &mut ToolsDataMap,
    are_tools_active: &mut bool,
    params: &mut AnnotationsParams,
    paths_navigator: &PathsNavigator,
) -> RvResult<()> {
    let skip_attributes = true;
    if params.tool_choice_delprop.is_some(skip_attributes) {
        ui.separator();
        ui.heading("Annotations per Folder");
        ui.label(egui::RichText::new(
            "Your project's content is shown below.",
        ));

        slider(
            ui,
            are_tools_active,
            &mut params.parents_depth,
            1..=5u8,
            "# subfolders to aggregate",
        );
        let max_n_folders = 5;
        params.tool_choice_delprop.run_mut(
            ui,
            tdm,
            |ui, tdm| {
                tdm_instance_annos(
                    BBOX_NAME,
                    tdm,
                    ui,
                    FolderParams {
                        max_n_folders,
                        parents_depth: params.parents_depth,
                    },
                    |ts| ts.bbox().map(|d| &d.annotations_map),
                    |ts| ts.bbox_mut().map(|d| &mut d.annotations_map),
                )
            },
            |ui, tdm| {
                tdm_instance_annos(
                    BRUSH_NAME,
                    tdm,
                    ui,
                    FolderParams {
                        max_n_folders,
                        parents_depth: params.parents_depth,
                    },
                    |ts| ts.brush().map(|d| &d.annotations_map),
                    |ts| ts.brush_mut().map(|d| &mut d.annotations_map),
                )
            },
            |_, _| Ok(()),
        )?;
        ui.separator();
        params.filter_relation_deletion = filter_relations_menu(
            "Delete Annotations from Files",
            ui,
            params.filter_relation_deletion,
        );
        let txt = params.filter_relation_deletion.select(
            "Log names of files in the file list that contain annotations",
            "Log names of files missing from the file list that contain annotations",
        );
        if ui.button(txt).clicked() {
            let filepaths = paths_navigator
                .paths_selector()
                .map(|ps| ps.filtered_idx_file_paths_pairs());
            if let Some(filepaths) = filepaths {
                let absent_files = get_all_files(
                    tdm,
                    &filepaths,
                    params.tool_choice_delprop,
                    params.filter_relation_deletion,
                )?;

                if absent_files.is_empty() {
                    tracing::info!("no relevant files with annotations found");
                }
                for (_, af, tool_name, count) in absent_files {
                    tracing::info!("file {af} has {count} {tool_name} annotations");
                }
            }
        }
        let txt = params.filter_relation_deletion.select(
            "Delete annotations of files in the file list",
            "Delete annotations of files missing from the file list",
        );

        if ui
            .button(txt)
            .on_hover_text("Are you sure? Double click!💀")
            .double_clicked()
        {
            let filepaths = paths_navigator
                .paths_selector()
                .map(|ps| ps.filtered_idx_file_paths_pairs());
            if let Some(filepaths) = filepaths {
                let absent_files = get_all_files(
                    tdm,
                    &filepaths,
                    params.tool_choice_delprop,
                    FilterRelation::Missing,
                )?;
                let absent_files = absent_files
                    .into_iter()
                    .map(|(_, af, tn, _)| (af.to_string(), tn))
                    .collect::<Vec<_>>();
                if absent_files.is_empty() {
                    tracing::info!("no missing annotations to delete")
                }
                for (af, tool_name) in absent_files {
                    tracing::info!("deleting annotations of {af} for tool {tool_name}");
                    if tool_name == BBOX_NAME {
                        let tools_data = tdm.get_mut(tool_name);
                        if let Some(td) = tools_data {
                            td.specifics.bbox_mut()?.annotations_map.remove(&af);
                        }
                    }
                    if tool_name == BRUSH_NAME {
                        let tools_data = tdm.get_mut(tool_name);
                        if let Some(td) = tools_data {
                            td.specifics.brush_mut()?.annotations_map.remove(&af);
                        }
                    }
                }
            }
        }
        ui.separator();
    }
    let skip_attributes = false;
    if params.tool_choice_delprop.is_some(skip_attributes) {
        ui.heading("Propagate to or Delete Annotations from Subsequent Images");
        if let Some(selected_file_idx) = paths_navigator.file_label_selected_idx() {
            egui::Grid::new("del-prop-grid")
                .num_columns(2)
                .show(ui, |ui| {
                    let n_prop: Option<usize> = ui_util::button_triggerable_number(
                        ui,
                        &mut params.text_buffers.label_propagation,
                        are_tools_active,
                        "propagate labels",
                        "number of following images to propagate label to",
                        Some("Double click! Annotations will be overriden if already existent!\n\
                              Image shapes as part of annotation information will also be propagated! 💀"),
                    );
                    ui.end_row();
                    let n_del: Option<usize> = ui_util::button_triggerable_number(
                        ui,
                        &mut params.text_buffers.label_deletion,
                        are_tools_active,
                        "delete labels",
                        "number of following images to delete label from",
                        Some("Double click! Annotations will be deleted! 💀"),
                    );
                    if let Some(ps) = paths_navigator.paths_selector() {
                        if let Some(n_prop) = n_prop {
                            let end = (selected_file_idx + n_prop).min(ps.len_filtered());
                            let range = selected_file_idx..end;
                            let paths = &ps.filtered_file_paths()[range];
                            if !paths.is_empty() {
                                tracing::info!(
                                    "propagating {} labels from {}",
                                    paths.len(),
                                    paths[0].path_relative()
                                );
                                trace_ok_err(params.tool_choice_delprop.run_mut(
                                    ui,
                                    tdm,
                                    |_, tdm| propagate_annos_of_tool(tdm, BBOX_NAME, paths),
                                    |_, tdm| propagate_annos_of_tool(tdm, BRUSH_NAME, paths),
                                    |_, tdm| propagate_annos_of_tool(tdm, ATTRIBUTES_NAME, paths),
                                ));
                            }
                        }
                        if let Some(n_del) = n_del {
                            let end = (selected_file_idx + n_del).min(ps.len_filtered());
                            let range = selected_file_idx..end;
                            let paths = &ps.filtered_file_paths()[range];
                            if !paths.is_empty() {
                                tracing::info!(
                                    "deleting {} labels from {}",
                                    paths.len(),
                                    paths[0].path_relative()
                                );
                                trace_ok_err(params.tool_choice_delprop.run_mut(
                                    ui,
                                    tdm,
                                    |_, tdm| delete_subsequent_annos_of_tool(tdm, BBOX_NAME, paths),
                                    |_, tdm| {
                                        delete_subsequent_annos_of_tool(tdm, BRUSH_NAME, paths)
                                    },
                                    |_, tdm| {
                                        delete_subsequent_annos_of_tool(tdm, ATTRIBUTES_NAME, paths)
                                    },
                                ));
                            }
                        }
                    }
                });
        } else {
            ui.label("no file selected");
        }
    }
    Ok(())
}

#[derive(Default, Clone, Debug)]
pub struct AnnoStatsRecord {
    tool_name: &'static str,
    cat_name: String,
    count: u64,
    count_per_file: f64,
    n_files_filtered_thistool_anycat: usize,
}
impl AnnoStatsRecord {
    pub fn cats_to_records(
        cat_to_count_map: &HashMap<(&'static str, usize), usize>,
        label_info: &LabelInfo,
        n_files_bbox: usize,
        n_files_brush: usize,
    ) -> Vec<Self> {
        let mut res = vec![Self::default(); cat_to_count_map.len()];

        for (i, ((tool_name, cat_idx), count)) in cat_to_count_map.iter().enumerate() {
            let n_files_filtered_thistool_anycat = if *tool_name == BBOX_NAME {
                n_files_bbox
            } else if *tool_name == BRUSH_NAME {
                n_files_brush
            } else {
                0
            };
            res[i] = AnnoStatsRecord {
                tool_name,
                cat_name: label_info.labels()[*cat_idx].clone(),
                count: *count as u64,
                count_per_file: *count as f64 / n_files_filtered_thistool_anycat as f64,
                n_files_filtered_thistool_anycat,
            };
        }
        res
    }
}

fn count_annos(
    count_map: &mut HashMap<(&'static str, usize), usize>,
    tool_name: &'static str,
    cat_idxs: &[usize],
) {
    for cat_idx in cat_idxs {
        let count = count_map.get_mut(&(tool_name, *cat_idx));
        if let Some(count) = count {
            *count += 1;
        } else {
            count_map.insert((tool_name, *cat_idx), 1);
        }
    }
}

/// number of files with annotations of the respective tool
fn count_files_of_tool<T>(
    tool_name: &'static str,
    unwrap_specifics: impl Fn(&ToolSpecifics) -> RvResult<&AnnotationsMap<T>>,
    tdm: &ToolsDataMap,
    filepaths: &[(usize, &PathPair)],
) -> RvResult<usize>
where
    T: InstanceAnnotate,
{
    Ok(iter_files_of_instance_tool(
        tdm,
        filepaths,
        tool_name,
        unwrap_specifics,
        FilterRelation::Available,
    )?
    .count())
}

fn collect_stats(
    tdm: &ToolsDataMap,
    filepaths: &[(usize, &PathPair)],
    tool_choice: ToolChoice,
) -> RvResult<Vec<AnnoStatsRecord>> {
    tracing::info!("computation of stats triggered");
    let files = get_all_files(tdm, filepaths, tool_choice, FilterRelation::Available)?;
    let mut count_map_bbox = HashMap::new();
    let mut count_map_brush = HashMap::new();
    for (_, path_key, tool_name, _) in &files {
        let f_bbox = |tdm: &ToolsDataMap| {
            let annos = get_annos_from_tdm!(BBOX_NAME, tdm, path_key, bbox);
            if let Some(annos) = annos {
                count_annos(&mut count_map_bbox, BBOX_NAME, annos.cat_idxs());
            }
            Ok(())
        };
        let f_brush = |tdm: &ToolsDataMap| {
            let annos = get_annos_from_tdm!(BRUSH_NAME, tdm, path_key, brush);
            if let Some(annos) = annos {
                count_annos(&mut count_map_brush, BRUSH_NAME, annos.cat_idxs());
            }
            Ok(())
        };
        trace_ok_err(ToolChoice::run(tool_name, tdm, f_bbox, f_brush));
    }
    let li_bbox = get_labelinfo_from_tdm!(BBOX_NAME, tdm, bbox);
    let li_brush = get_labelinfo_from_tdm!(BRUSH_NAME, tdm, brush);
    let n_bbox_files = count_files_of_tool(
        BBOX_NAME,
        |ts| ts.bbox().map(|d| &d.annotations_map),
        tdm,
        filepaths,
    )?;
    let n_brush_files = count_files_of_tool(
        BRUSH_NAME,
        |ts| ts.brush().map(|d| &d.annotations_map),
        tdm,
        filepaths,
    )?;
    let mut bbox_records = li_bbox
        .map(|li| {
            AnnoStatsRecord::cats_to_records(&count_map_bbox, li, n_bbox_files, n_brush_files)
        })
        .unwrap_or_default();
    let brush_records = li_brush
        .map(|li| {
            AnnoStatsRecord::cats_to_records(&count_map_brush, li, n_bbox_files, n_brush_files)
        })
        .unwrap_or_default();
    bbox_records.extend(brush_records);
    bbox_records.sort_by_key(|elt| elt.count);
    bbox_records.reverse();
    tracing::info!("{} records collected", bbox_records.len());
    Ok(bbox_records)
}

fn anno_stats(
    ui: &mut Ui,
    tdm: &mut ToolsDataMap,
    stats_compute_results: &mut Option<Vec<AnnoStatsRecord>>,
    tool_choice: ToolChoice,
    paths_selector: Option<&PathsSelector>,
) -> RvResult<()> {
    let filepaths = paths_selector.map(|ps| ps.filtered_idx_file_paths_pairs());
    let skip_attributes = true;
    if !tool_choice.is_some(skip_attributes) {
        *stats_compute_results = None;
    } else {
        if ui.button("(Re-)compute stats of filtered files").clicked() {
            if let Some(filepaths) = filepaths {
                *stats_compute_results = Some(collect_stats(tdm, &filepaths, tool_choice)?);
            }
        }
        if let Some(stats_compute_results) = stats_compute_results {
            if !stats_compute_results.is_empty() {
                egui::Grid::new("anno-stats-records-")
                    .num_columns(4)
                    .show(ui, |ui| {
                        ui.label(RichText::new("tool").strong());
                        ui.label(RichText::new("category").strong());
                        ui.label(RichText::new("count").strong());
                        ui.label(RichText::new("mean count").strong());
                        ui.label(RichText::new("# files").strong()).on_hover_text(
                            "number of files in the filtered filelist that contain or contained any annotations of the respective tool",
                        );
                        for record in stats_compute_results.iter() {
                            ui.end_row();
                            ui.label(RichText::new(record.tool_name).monospace());
                            ui.label(RichText::new(&record.cat_name).monospace());
                            ui.label(RichText::new(format!("{}", record.count)).monospace());
                            ui.label(
                                RichText::new(format!("{:0.3}", record.count_per_file)).monospace(),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{}",
                                    record.n_files_filtered_thistool_anycat
                                ))
                                .monospace(),
                            );
                        }
                    });
            } else {
                ui.label("no annotations found");
            }
        }
    }
    Ok(())
}

fn anno_plots(
    ui_params: (&Context, &mut Ui),
    tdm: &ToolsDataMap,
    tool_choice: ToolChoice,
    paths_selector: Option<&PathsSelector>,
    are_tools_active: &mut bool,
    plot_params: (
        &mut HashMap<String, bool>,
        &mut HashMap<String, Vec<PlotPoint>>,
    ),
) -> RvResult<()> {
    let (selected_attributes, attribute_plots) = plot_params;
    let (ctx, ui) = ui_params;
    let atd = tdm
        .get(ATTRIBUTES_NAME)
        .ok_or_else(|| rverr!("{ATTRIBUTES_NAME} not initialized"))?
        .specifics
        .attributes()?;
    if tool_choice.attributes {
        ui.group(|ui| {
            ui.label("Select Attribute");
            for name in atd.attr_names() {
                if !selected_attributes.contains_key(name) {
                    selected_attributes.insert(name.clone(), false);
                }
                let attr = selected_attributes.get_mut(name);
                if let Some(attr) = attr {
                    ui.checkbox(attr, name);
                }
            }
        });
    }
    if ui.button("plot").clicked() {
        let filepaths = paths_selector
            .map(|ps| ps.filtered_idx_file_paths_pairs())
            .ok_or_else(|| rverr!("no file paths found"))?;
        *attribute_plots = HashMap::new();
        for (selected_attr, is_selected) in selected_attributes.iter() {
            if *is_selected {
                let mut plot = vec![];
                for (file_idx, attr_map) in iter_attributes_of_files(atd, &filepaths) {
                    let value = attr_map.get(selected_attr);
                    if let Some(value) = value {
                        let y = match value {
                            AttrVal::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                            AttrVal::Float(x) => *x,
                            AttrVal::Int(n) => n.map(|n| n as f64),
                            AttrVal::Str(s) => Some(s.len() as f64),
                        };
                        if let Some(y) = y {
                            plot.push(PlotPoint {
                                x: file_idx as f64,
                                y,
                            });
                        }
                    }
                }
                if !plot.is_empty() {
                    attribute_plots.insert(selected_attr.clone(), plot);
                }
            }
        }
        macro_rules! count_plot {
            ($tool_name:expr, $accessfunc:ident, $plotname:expr) => {
                let mut plot = vec![];
                for (fidx, _, _, count) in iter_files_of_instance_tool(
                    tdm,
                    &filepaths,
                    $tool_name,
                    |ts| ts.$accessfunc().map(|d| &d.annotations_map),
                    FilterRelation::Available,
                )? {
                    if let Some(fidx) = fidx {
                        plot.push(PlotPoint {
                            x: fidx as f64,
                            y: count as f64,
                        });
                    }
                }
                attribute_plots.insert($plotname.into(), plot);
            };
        }
        if tool_choice.bbox {
            count_plot!(BBOX_NAME, bbox, "Bbox counts");
        }
        if tool_choice.brush {
            count_plot!(BRUSH_NAME, brush, "Brush counts");
        }
    }
    if !attribute_plots.is_empty() {
        let x_fmt = move |x: GridMark, _range: &RangeInclusive<f64>| {
            if x.value.fract().abs() < 1e-6 {
                let i = x.value.round() as usize;
                let filelabel = paths_selector.and_then(|ps| ps.file_label_of_idx(i));
                filelabel.map(|s| s.to_string()).unwrap_or_default()
            } else {
                String::new()
            }
        };
        egui::Window::new("Annotation Plots")
            .collapsible(false)
            .show(ctx, |ui| {
                ui_with_deactivated_tools_on_hover(are_tools_active, || {
                    Plot::new("attribute plots")
                        .legend(Legend::default().position(Corner::LeftTop))
                        .x_axis_formatter(x_fmt)
                        .show(ui, |plot_ui| {
                            for (k, v) in attribute_plots.iter() {
                                plot_ui.points(
                                    Points::new(k, PlotPoints::Borrowed(v))
                                        .shape(MarkerShape::Circle)
                                        .radius(5.0)
                                        .filled(true),
                                );
                            }
                        })
                        .response
                });
            });
    }
    Ok(())
}

fn autosaves(ui: &mut Ui, ctrl: &mut Control, mut close: Close) -> (Close, Option<ToolsDataMap>) {
    let mut tdm = None;
    let (today, date_n_days_ago) = make_timespan(AUTOSAVE_KEEP_N_DAYS);
    let folder = Path::new(ctrl.cfg.home_folder());
    let files = trace_ok_err(list_files(folder, Some(date_n_days_ago), Some(today)));
    ui.heading("Reset Annotations to Autsave");
    egui::Grid::new("autosaves-menu-grid").show(ui, |ui| {
        ui.label(egui::RichText::new("name").monospace());
        ui.label(egui::RichText::new("size").monospace());
        ui.label(egui::RichText::new("modified").monospace());
        ui.end_row();
        if let Some(autosaves) = files {
            let cur_prj_path = ctrl.cfg.current_prj_path().to_path_buf();
            let stem = trace_ok_err(file_util::to_stem_str(&cur_prj_path))
                .unwrap_or("default")
                .to_string();
            let files = iter::once(cur_prj_path).chain(autosaves.into_iter().filter(|p| {
                p.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.starts_with(&stem))
                    == Some(true)
            }));
            let fileinfos = files.clone().map(|path| fileinfo(&path));

            let mut combined: Vec<_> = files
                .zip(fileinfos)
                .flat_map(|(file, info)| info.map(|i| (file, i)))
                .collect();
            combined.sort_by(|(_, (_, datetime1)), (_, (_, datetime2))| datetime1.cmp(datetime2));

            for (path, (mb, datetime)) in combined.iter().rev() {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if ui
                        .button(egui::RichText::new(file_name).monospace())
                        .on_hover_text("double click to apply, LOSS(💀) of unsaved data")
                        .double_clicked()
                    {
                        tdm = trace_ok_err(ctrl.replace_with_save(path));
                        close = Close::Yes;
                    }
                    ui.label(egui::RichText::new(mb).monospace());
                    ui.label(egui::RichText::new(datetime).monospace());
                    ui.end_row();
                }
            }
        }
    });
    (close, tdm)
}

fn annotations_popup(
    ctx: &Context,
    ui: &mut Ui,
    ctrl: &mut Control,
    in_tdm: &mut ToolsDataMap,
    are_tools_active: &mut bool,
    anno_params: &mut AnnotationsParams,
) -> (Close, Option<ToolsDataMap>) {
    let mut close = Close::No;
    let mut tdm = None;
    Frame::popup(ui.style()).show(ui, |ui| {
        if ui.button("Close").clicked() {
            close = Close::Yes;
        }
        ui.separator();
        egui::CollapsingHeader::new("Restore Annotations").show(ui, |ui| {
            (close, tdm) = autosaves(ui, ctrl, close);
        });
        ui.separator();
        egui::CollapsingHeader::new("Delete or Propagate Annotations").show(ui, |ui| {
            let skip_attrs = false;
            anno_params.tool_choice_delprop.ui(ui, skip_attrs);
            trace_ok_err(annotations(
                ui,
                in_tdm,
                are_tools_active,
                anno_params,
                &ctrl.paths_navigator,
            ));
        });
        ui.separator();
        egui::CollapsingHeader::new("Annotation Statistics").show(ui, |ui| {
            let skip_attrs = true;
            anno_params.tool_choice_stats.ui(ui, skip_attrs);
            trace_ok_err(anno_stats(
                ui,
                in_tdm,
                &mut anno_params.stats_result,
                anno_params.tool_choice_stats,
                ctrl.paths_navigator.paths_selector(),
            ));
        });
        ui.separator();
        egui::CollapsingHeader::new("Plot images vs. annotations").show(ui, |ui| {
            let skip_attrs = false;
            anno_params.tool_choice_plot.ui(ui, skip_attrs);
            trace_ok_err(anno_plots(
                (ctx, ui),
                in_tdm,
                anno_params.tool_choice_plot,
                ctrl.paths_navigator.paths_selector(),
                are_tools_active,
                (
                    &mut anno_params.selected_attributes_for_plot,
                    &mut anno_params.attribute_plots,
                ),
            ));
        });
        ui.separator();
        if ui.button("Close").clicked() {
            close = Close::Yes;
        }
    });
    (close, tdm)
}

pub struct AutosaveMenu<'a> {
    ctx: &'a Context,
    id: Id,
    ctrl: &'a mut Control,
    tdm: &'a mut ToolsDataMap,
    project_loaded: &'a mut bool,
    are_tools_active: &'a mut bool,
    anno_params: &'a mut AnnotationsParams,
}
impl<'a> AutosaveMenu<'a> {
    pub fn new(
        ctx: &'a Context,
        id: Id,
        ctrl: &'a mut Control,
        tools_data_map: &'a mut ToolsDataMap,
        project_loaded: &'a mut bool,
        are_tools_active: &'a mut bool,
        anno_params: &'a mut AnnotationsParams,
    ) -> AutosaveMenu<'a> {
        Self {
            ctx,
            id,
            ctrl,
            tdm: tools_data_map,
            project_loaded,
            are_tools_active,
            anno_params,
        }
    }
}
impl Widget for AutosaveMenu<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        *self.project_loaded = false;
        let autosaves_btn_resp = ui.button("Annotations");
        if autosaves_btn_resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(self.id));
        }
        if ui.memory(|m| m.is_popup_open(self.id)) {
            let area = Area::new(self.id)
                .order(Order::Foreground)
                .default_pos(autosaves_btn_resp.rect.left_bottom());

            let mut close = Close::No;
            let area_response = area
                .show(ui.ctx(), |ui| {
                    let (close_, tdm) = annotations_popup(
                        self.ctx,
                        ui,
                        self.ctrl,
                        self.tdm,
                        self.are_tools_active,
                        self.anno_params,
                    );
                    close = close_;
                    if let Some(tdm) = tdm {
                        *self.tdm = tdm;
                        *self.project_loaded = true;
                    }
                })
                .response;
            if let Close::Yes = close {
                ui.memory_mut(|m| m.toggle_popup(self.id));
            }
            if !autosaves_btn_resp.clicked() && area_response.clicked_elsewhere() {
                ui.memory_mut(|m| m.toggle_popup(self.id));
            }
        }
        autosaves_btn_resp
    }
}

#[cfg(test)]
use crate::test_helpers;

#[test]
fn test_counts() {
    let tf = test_helpers::get_test_folder();
    let test_file_src_1 = tf.join(tf.join("import-test-src-flowerlabel.json"));
    let tdm = test_helpers::prj_load(file_util::path_to_str(&test_file_src_1).unwrap());
    let filepath = PathPair::new(
        "/Users/b/Desktop/tmp/flower.jpg".to_string(),
        &test_file_src_1,
    );
    let records = collect_stats(
        &tdm,
        &[(34, &filepath)],
        ToolChoice {
            bbox: true,
            brush: true,
            attributes: false,
        },
    )
    .unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].count, 1);
    assert_eq!(records[1].count, 1);
    let records = collect_stats(
        &tdm,
        &[(34, &filepath)],
        ToolChoice {
            bbox: true,
            brush: false,
            attributes: false,
        },
    )
    .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].count, 1);
    let records = collect_stats(
        &tdm,
        &[(34, &filepath)],
        ToolChoice {
            bbox: false,
            brush: true,
            attributes: true,
        },
    )
    .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].count, 1);
}
