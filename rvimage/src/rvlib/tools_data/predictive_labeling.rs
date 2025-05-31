use serde::{Deserialize, Serialize};

use super::parameters::{ParamMap, ParamVal};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct PredictiveLabelingData {
    pub new_param_name_buffer: String,
    pub new_param_val_buffer: ParamVal,
    pub param_buffers: Vec<String>,
    pub parameters: ParamMap,
    pub url: String,
    pub authorization_headers: Option<String>,
    pub is_prediction_triggered: bool,
}
