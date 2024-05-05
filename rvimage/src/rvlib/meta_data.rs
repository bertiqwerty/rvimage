use serde::{Deserialize, Serialize};

use crate::cfg::{AzureBlobCfg, PyHttpReaderCfg, SshCfg};

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, Eq)]
pub enum ConnectionData {
    Ssh(SshCfg),
    PyHttp(PyHttpReaderCfg),
    #[cfg(feature = "azure_blob")]
    AzureBlobCfg(AzureBlobCfg),
    #[default]
    None,
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct MetaData {
    pub file_path: Option<String>,
    pub file_selected_idx: Option<usize>,
    pub connection_data: ConnectionData,
    pub ssh_cfg: Option<SshCfg>,
    pub opened_folder: Option<String>,
    pub export_folder: Option<String>,
    pub is_loading_screen_active: Option<bool>,
    pub is_file_list_empty: Option<bool>,
}
impl MetaData {
    pub fn from_filepath(file_path: String, file_selected_idx: usize) -> Self {
        MetaData {
            file_path: Some(file_path),
            file_selected_idx: Some(file_selected_idx),
            connection_data: ConnectionData::None,
            ssh_cfg: None,
            opened_folder: None,
            export_folder: None,
            is_loading_screen_active: None,
            is_file_list_empty: None,
        }
    }
}
