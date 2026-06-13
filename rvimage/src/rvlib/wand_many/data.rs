use serde::{Deserialize, Serialize};

use crate::parameters::ParamMap;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct WandManyMessage {
    pub comment: String,
    pub response: Option<String>,
    pub success_assessment: Option<u8>,
}
impl WandManyMessage {
    pub fn from_comment(cmt: String) -> Self {
        Self {
            comment: cmt,
            response: None,
            success_assessment: None,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct WandManyData {
    pub messages: Vec<WandManyMessage>,
    pub params: Option<ParamMap>,
    pub subfolders_to_exclude: Vec<String>,
}
