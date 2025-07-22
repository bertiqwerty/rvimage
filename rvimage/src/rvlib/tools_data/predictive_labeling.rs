use std::{collections::HashMap, time::Instant};

use serde::{Deserialize, Serialize};

use crate::tools::{BBOX_NAME, BRUSH_NAME};

use super::parameters::{ParamMap, ParamVal};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PredictiveLabelingData {
    pub new_param_name_buffer: String,
    pub new_param_val_buffer: ParamVal,
    pub param_buffers: Vec<String>,
    pub parameters: ParamMap,
    pub url: String,
    pub authorization_headers: Option<String>,
    pub tool_labelnames_map: HashMap<String, Vec<String>>,
    pub timeout_ms: usize,
    #[serde(default)]
    pub timeout_buffer: String,
    #[serde(skip)]
    trigger: Option<(bool, Instant)>,
    #[serde(skip)]
    pub to_be_removed: Option<usize>,
}

impl PredictiveLabelingData {
    pub fn trigger_prediction(&mut self) {
        self.trigger = Some((true, Instant::now()));
    }
    pub fn untrigger(&mut self) {
        self.trigger = self.trigger.map(|(_, t)| (false, t));
    }
    pub fn prediction_start_triggered(&self) -> bool {
        self.trigger.map(|(start_prediction, _)| start_prediction) == Some(true)
    }
    pub fn trigger_time(&self) -> Option<&Instant> {
        self.trigger.as_ref().map(|(_, t)| t)
    }
    pub fn kill_trigger(&mut self) {
        self.trigger = None;
    }
}

impl Default for PredictiveLabelingData {
    fn default() -> Self {
        Self {
            new_param_name_buffer: String::default(),
            new_param_val_buffer: ParamVal::default(),
            param_buffers: Vec::default(),
            parameters: ParamMap::default(),
            url: "http".into(),
            authorization_headers: None,
            tool_labelnames_map: HashMap::from([
                (BBOX_NAME.into(), vec![]),
                (BRUSH_NAME.into(), vec![]),
            ]),
            timeout_ms: 30000,
            timeout_buffer: "".into(),
            trigger: None,
            to_be_removed: None,
        }
    }
}

impl PartialEq for PredictiveLabelingData {
    fn eq(&self, other: &Self) -> bool {
        self.authorization_headers == other.authorization_headers
            && self.tool_labelnames_map == other.tool_labelnames_map
            && self.parameters == other.parameters
            && self.url == other.url
            && self.timeout_ms == other.timeout_ms
    }
}
