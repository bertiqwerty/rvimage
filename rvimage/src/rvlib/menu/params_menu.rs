use egui::Ui;

use crate::{
    menu::ui_util::{
        removable_rows, text_edit_multiline, text_edit_singleline, update_numeric_attribute,
    },
    parameters::{ParamMap, ParamVal},
};

pub const FLOAT_LABEL: &str = "Float";
pub const INT_LABEL: &str = "Int";
pub const TEXT_LABEL: &str = "Text";
pub const BOOL_LABEL: &str = "Bool";
pub fn add_parameter_menu<'a>(
    ui: &mut Ui,
    mut new_param_name: String,
    mut new_param_val: ParamVal,
    mut existing_param_names: impl Iterator<Item = &'a String>,
    are_tools_active: &mut bool,
) -> (String, ParamVal, bool) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_label("")
            .selected_text(format!(
                "{:?}",
                match new_param_val {
                    ParamVal::Float(_) => FLOAT_LABEL,
                    ParamVal::Int(_) => INT_LABEL,
                    ParamVal::Str(_) => TEXT_LABEL,
                    ParamVal::Bool(_) => BOOL_LABEL,
                }
            ))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut new_param_val, ParamVal::Float(None), FLOAT_LABEL);
                ui.selectable_value(&mut new_param_val, ParamVal::Int(None), INT_LABEL);
                ui.selectable_value(&mut new_param_val, ParamVal::Str(String::new()), TEXT_LABEL);
                ui.selectable_value(&mut new_param_val, ParamVal::Bool(false), BOOL_LABEL);
            });
    });
    text_edit_singleline(ui, &mut new_param_name, are_tools_active);
    if ui.button("add").clicked() {
        if existing_param_names.any(|pm| pm == &new_param_name) {
            tracing::error!(
                "attribute {:?} already exists, we do not re-create it",
                new_param_name
            );
            (new_param_name, new_param_val, false)
        } else {
            // only case where we add a new attribute and hence return true
            (new_param_name, new_param_val, true)
        }
    } else {
        (new_param_name, new_param_val, false)
    }
}

#[derive(Default)]
pub enum ExistingParamMenuAction {
    Rename(usize),
    Remove(usize),
    #[default]
    None,
}

#[derive(Default)]
pub struct ExistingParamMenuResult {
    pub action: ExistingParamMenuAction,
    pub buffers: Vec<String>,
    pub is_update_triggered: bool,
    pub param_map: ParamMap,
}

pub fn existing_params_menu(
    ui: &mut Ui,
    mut attr_map: ParamMap,
    are_tools_active: &mut bool,
    mut more_cols: impl FnMut(&mut Ui, bool, usize, ParamVal) -> bool,
    mut param_buffers: Vec<String>,
) -> ExistingParamMenuResult {
    let mut result = ExistingParamMenuResult::default();
    let n_attrs = attr_map.len();
    egui::Grid::new("attributes_grid")
        .num_columns(4)
        .show(ui, |ui| {
            let to_be_removed = removable_rows(ui, n_attrs, |ui, idx_row| {
                let attr_name = attr_map
                    .keys()
                    .nth(idx_row)
                    .unwrap_or_else(|| {
                        panic!("BUG! could not find idx {idx_row} in params of {attr_map:?}")
                    })
                    .clone();
                if let Some(param_buffer) = param_buffers.get_mut(idx_row) {
                    ui.label(&attr_name);
                    let mut input_changed = false;
                    match attr_map.get_mut(&attr_name) {
                        Some(ParamVal::Bool(b)) => {
                            if ui.checkbox(b, "").changed() {
                                input_changed = true;
                            }
                        }
                        Some(ParamVal::Float(x)) => {
                            input_changed = update_numeric_attribute(
                                ui,
                                are_tools_active,
                                x,
                                FLOAT_LABEL,
                                param_buffer,
                            );
                        }
                        Some(ParamVal::Int(x)) => {
                            input_changed = update_numeric_attribute(
                                ui,
                                are_tools_active,
                                x,
                                INT_LABEL,
                                param_buffer,
                            );
                        }
                        Some(ParamVal::Str(s)) => {
                            input_changed = text_edit_singleline(ui, s, are_tools_active)
                                .on_hover_text(TEXT_LABEL)
                                .lost_focus();
                        }
                        None => {
                            tracing::warn!("attr_map does not contain {attr_name}");
                        }
                    }
                    if let Some(attr_val) = attr_map.get(&attr_name)
                        && more_cols(ui, input_changed, idx_row, attr_val.clone())
                    {
                        result.is_update_triggered = true;
                    }

                    if ui.button("rename").clicked() {
                        result.action = ExistingParamMenuAction::Rename(idx_row);
                        result.is_update_triggered = true;
                    }
                    ui.end_row();
                }
            });
            result.buffers = param_buffers;
            if let Some(tbr) = to_be_removed {
                result.action = ExistingParamMenuAction::Remove(tbr);
            }
        });
    for (name, val) in attr_map.iter_mut() {
        if let ParamVal::Str(s) = val {
            egui::CollapsingHeader::new(format!("Edit {name}")).show(ui, |ui| {
                let input_changed = text_edit_multiline(ui, s, are_tools_active)
                    .on_hover_text(TEXT_LABEL)
                    .lost_focus();
                if input_changed {
                    result.is_update_triggered = true;
                }
            });
        }
    }
    result.param_map = attr_map;
    result
}
