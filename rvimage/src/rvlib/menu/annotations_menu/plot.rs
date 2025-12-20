use std::{cmp::Ordering, collections::HashMap, ops::RangeInclusive};

use egui::Ui;
use egui_plot::{Corner, GridMark, Legend, MarkerShape, Plot, PlotPoint, PlotPoints, Points};
use rvimage_domain::{PtF, RvResult, rverr};

use crate::{
    InstanceAnnotate, ToolsDataMap, get_labelinfo_from_tdm, get_specifics_from_tdm,
    menu::ui_util::{process_number, ui_with_deactivated_tools_on_hover},
    paths_selector::PathsSelector,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
    tools_data::{AccessInstanceData, LabelInfo, PlotAnnotationStats},
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

fn predicate_always_true<T>(_: &T) -> bool {
    true
}
fn predicate_area_belowth<T>(anno: &T, area_threshold: f64) -> bool
where
    T: InstanceAnnotate,
{
    let shape = anno.enclosing_bb().shape();
    shape.w * shape.h <= area_threshold
}

pub(super) fn anno_plots<'a>(
    ui: &'a mut Ui,
    tdm: &ToolsDataMap,
    tool_choice: ToolChoice,
    paths_selector: Option<&'a PathsSelector>,
    are_tools_active: &'a mut bool,
    plot_params: (
        Selection<'a>,
        &'a mut HashMap<String, Vec<PlotPoint>>,
        &mut bool,
        &mut f64,
        &mut bool,
        &mut String,
    ),
) -> RvResult<Option<usize>> {
    let (selection, attribute_plots, window_open, area_threshold, area_restricted, area_th_buffer) =
        plot_params;
    let selected_attributes = selection.attributes;
    let selected_bboxclasses = selection.bbox_classes;
    let selected_brushclasses = selection.brush_classes;
    let atd = tdm
        .get_specifics(ATTRIBUTES_NAME)
        .and_then(|d| d.attributes().ok());
    if let Some(atd) = atd
        && tool_choice.attributes
    {
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
    if tool_choice.is_some(true) {
        ui.checkbox(area_restricted, "restrict area with upper bound");
        if *area_restricted {
            if let (_, Some(at)) =
                process_number(ui, are_tools_active, "enter maximum area", area_th_buffer)
            {
                *area_threshold = at;
            }
        } else {
            *area_threshold = f64::MAX;
        }
    }
    if tool_choice.bbox {
        class_selection(ui, BBOX_NAME, bbox_labelinfo, selected_bboxclasses);
    }
    let brush_labelinfo = get_labelinfo_from_tdm!(BRUSH_NAME, tdm, brush);
    if tool_choice.brush {
        class_selection(ui, BRUSH_NAME, brush_labelinfo, selected_brushclasses);
    }
    if ui.button("plot").clicked() {
        *window_open = true;
        let filepaths = paths_selector
            .map(|ps| ps.filtered_idx_file_paths_pairs())
            .ok_or_else(|| rverr!("no file paths found"))?;
        *attribute_plots = HashMap::new();

        macro_rules! plot_instance {
            ($tool_name:expr, $accessfunc:ident, $selected:expr, $pred:expr) => {
                let data = get_specifics_from_tdm!($tool_name, tdm, $accessfunc);
                if let Some(d) = data {
                    let plots = d.plot($selected, &filepaths, $pred)?;
                    for (classname, plot) in plots {
                        attribute_plots.insert(classname, plot_to_egui(plot));
                    }
                }
            };
        }
        if tool_choice.attributes {
            plot_instance!(
                ATTRIBUTES_NAME,
                attributes,
                selected_attributes,
                &predicate_always_true
            );
        }
        if tool_choice.bbox {
            plot_instance!(BBOX_NAME, bbox, selected_bboxclasses, &|anno| {
                predicate_area_belowth(anno, *area_threshold)
            });
        }
        if tool_choice.brush {
            plot_instance!(BRUSH_NAME, brush, selected_brushclasses, &|anno| {
                predicate_area_belowth(anno, *area_threshold)
            });
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

        let mut pointer_pos = None;
        let mut plot_response = None;
        const PLOT_POINT_RADIUS: f32 = 5.0;
        egui::Window::new("Annotation Plots")
            .collapsible(false)
            .open(window_open)
            .movable(true)
            .show(ui.ctx(), |ui| {
                ui_with_deactivated_tools_on_hover(are_tools_active, || {
                    let plot_response_ = Plot::new("attribute plots")
                        .legend(Legend::default().position(Corner::LeftTop))
                        .x_axis_formatter(x_fmt)
                        .show(ui, |plot_ui| {
                            pointer_pos = plot_ui.pointer_coordinate();

                            for (k, v) in attribute_plots.iter() {
                                plot_ui.points(
                                    Points::new(k, PlotPoints::Borrowed(v))
                                        .shape(MarkerShape::Circle)
                                        .radius(PLOT_POINT_RADIUS)
                                        .filled(true),
                                );
                            }
                        });
                    let r = plot_response_.response.clone();
                    plot_response = Some(plot_response_);
                    r
                });
            });
        if plot_response.map(|r| r.response.clicked()) == Some(true)
            && let Some(pos) = pointer_pos
        {
            let dist = |v: PlotPoint, p: PlotPoint| (v.x - p.x).powi(2) + (v.y - p.y).powi(2);
            let data_point_close_to_mouse = attribute_plots
                .iter()
                .flat_map(|(_, plt)| plt.iter())
                .filter(|v| (v.x - pos.x).abs() <= PLOT_POINT_RADIUS as f64)
                .min_by(|v1, v2| {
                    let dist1 = dist(**v1, pos);
                    let dist2 = dist(**v2, pos);
                    match dist1.partial_cmp(&dist2) {
                        Some(o) => o,
                        None => Ordering::Less,
                    }
                });
            let index = data_point_close_to_mouse.map(|p| p.x.round() as usize);
            return Ok(index);
        }
    }
    Ok(None)
}
