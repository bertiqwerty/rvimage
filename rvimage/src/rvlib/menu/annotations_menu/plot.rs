use std::{collections::HashMap, ops::RangeInclusive};

use egui::{Context, Ui};
use egui_plot::{Corner, GridMark, Legend, MarkerShape, Plot, PlotPoint, PlotPoints, Points};
use rvimage_domain::{rverr, PtF, RvResult};

use crate::{
    get_labelinfo_from_tdm, get_specifics_from_tdm,
    menu::ui_util::ui_with_deactivated_tools_on_hover,
    paths_selector::PathsSelector,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
    tools_data::{AccessInstanceData, LabelInfo, PlotAnnotationStats},
    ToolsDataMap,
};

use super::core::ToolChoice;

fn plot_to_egui(plot: Vec<PtF>) -> Vec<PlotPoint> {
    plot.into_iter()
        .map(|p| PlotPoint { x: p.x, y: p.y })
        .collect()
}

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

        macro_rules! plot_instance {
            ($tool_name:expr, $accessfunc:ident, $selected:expr) => {
                let data = get_specifics_from_tdm!($tool_name, tdm, $accessfunc);
                if let Some(d) = data {
                    let plots = d.plot($selected, &filepaths)?;
                    for (classname, plot) in plots {
                        attribute_plots.insert(classname, plot_to_egui(plot));
                    }
                }
            };
        }
        if tool_choice.attributes {
            plot_instance!(ATTRIBUTES_NAME, attributes, selected_attributes);
        }
        if tool_choice.bbox {
            plot_instance!(BBOX_NAME, bbox, selected_bboxclasses);
        }
        if tool_choice.brush {
            plot_instance!(BRUSH_NAME, brush, selected_brushclasses);
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
