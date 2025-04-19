use std::{collections::HashMap, ops::RangeInclusive};

use egui::{Context, Ui};
use egui_plot::{Corner, GridMark, Legend, MarkerShape, Plot, PlotPoint, PlotPoints, Points};
use rvimage_domain::{rverr, RvResult};

use crate::{
    menu::{
        annotations_menu::iter_attributes_of_files, ui_util::ui_with_deactivated_tools_on_hover,
    },
    paths_selector::PathsSelector,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
    tools_data::attributes_data::AttrVal,
    ToolsDataMap,
};

use super::core::{iter_files_of_instance_tool, FilterRelation, ToolChoice};

pub(super) fn anno_plots(
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
    if tool_choice.bbox {
        ui.group(|ui| {
            ui.label("Select Class");
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
                    None,
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
