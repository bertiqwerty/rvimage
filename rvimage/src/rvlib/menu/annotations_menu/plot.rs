use std::{collections::HashMap, ops::RangeInclusive};

use egui::{Context, Ui};
use egui_plot::{Corner, GridMark, Legend, MarkerShape, Plot, PlotPoint, PlotPoints, Points};
use rvimage_domain::{rverr, RvResult};

use crate::{
    get_labelinfo_from_tdm,
    menu::{
        annotations_menu::iter_attributes_of_files, ui_util::ui_with_deactivated_tools_on_hover,
    },
    paths_selector::PathsSelector,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
    tools_data::{attributes_data::AttrVal, ExportAsCoco, LabelInfo},
    ToolsDataMap,
};

use super::core::{iter_files_of_instance_tool, FilterRelation, ToolChoice};

pub(super) struct Selection<'a> {
    pub attributes: &'a mut HashMap<String, bool>,
    pub bbox_classes: &'a mut HashMap<String, bool>,
    pub brush_classes: &'a mut HashMap<String, bool>,
}

fn class_selection(
    ui: &mut Ui,
    tool_name: &str,
    labelinfo: Option<&LabelInfo>,
    selected: &mut HashMap<String, bool>,
) {
    ui.collapsing(format!("Select classes of {tool_name}"), |ui| {
        if let Some(labelinfo) = labelinfo {
            for name in labelinfo.labels() {
                if !selected.contains_key(name) {
                    selected.insert(name.clone(), false);
                }
                let is_class_selected = selected.get_mut(name);
                if let Some(is_selected) = is_class_selected {
                    ui.checkbox(is_selected, name);
                }
            }
        }
    });
}

pub(super) fn anno_plots<'a>(
    ui_params: (&Context, &'a mut Ui),
    tdm: &ToolsDataMap,
    tool_choice: ToolChoice,
    paths_selector: Option<&'a PathsSelector>,
    are_tools_active: &'a mut bool,
    plot_params: (Selection<'a>, &'a mut HashMap<String, Vec<PlotPoint>>),
) -> RvResult<()> {
    let (selection, attribute_plots) = plot_params;
    let selected_attributes = selection.attributes;
    let selected_bboxclasses = selection.bbox_classes;
    let selected_brushclasses = selection.brush_classes;
    let (ctx, ui) = ui_params;
    let atd = tdm
        .get(ATTRIBUTES_NAME)
        .ok_or_else(|| rverr!("{ATTRIBUTES_NAME} not initialized"))?
        .specifics
        .attributes()?;
    if tool_choice.attributes {
        ui.collapsing("Select Attributes", |ui| {
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
    let bbox_labelinfo = get_labelinfo_from_tdm!(BBOX_NAME, tdm, bbox);
    if tool_choice.bbox {
        class_selection(ui, BBOX_NAME, bbox_labelinfo, selected_bboxclasses);
    }
    let brush_labelinfo = get_labelinfo_from_tdm!(BRUSH_NAME, tdm, brush);
    if tool_choice.brush {
        class_selection(ui, BRUSH_NAME, brush_labelinfo, selected_brushclasses);
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
                    attribute_plots.insert(format!("{ATTRIBUTES_NAME}_{selected_attr}"), plot);
                }
            }
        }

        macro_rules! count_plot {
            ($tool_name:expr, $accessfunc:ident, $selected:expr, $labelinfo:expr) => {
                let relevant_indices = if let Some(labelinfo) = $labelinfo {
                    $selected
                        .iter()
                        .filter(|(_, is_selected)| **is_selected)
                        .flat_map(|(selected_label, _)| {
                            labelinfo
                                .labels()
                                .iter()
                                .position(|label| label == selected_label)
                        })
                        .collect::<Vec<_>>()
                } else {
                    vec![]
                };
                let mut plot = vec![];
                for (fidx, _, _, count) in iter_files_of_instance_tool(
                    tdm,
                    &filepaths,
                    $tool_name,
                    |ts| ts.$accessfunc().map(|d| &d.annotations_map),
                    FilterRelation::Available,
                    if relevant_indices.is_empty() {
                        None
                    } else {
                        Some(&relevant_indices)
                    },
                )? {
                    if let Some(fidx) = fidx {
                        plot.push(PlotPoint {
                            x: fidx as f64,
                            y: count as f64,
                        });
                    }
                }
                attribute_plots.insert(format!("{}", $tool_name), plot);
            };
        }
        if tool_choice.bbox {
            count_plot!(BBOX_NAME, bbox, selected_bboxclasses, bbox_labelinfo);
        }
        if tool_choice.brush {
            count_plot!(BRUSH_NAME, brush, selected_brushclasses, brush_labelinfo);
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
