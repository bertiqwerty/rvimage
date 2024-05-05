use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    cfg::{AzureBlobCfg, PyHttpReaderCfg, SshCfg},
    file_util::tf_to_annomap_key,
};

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
    file_path_relative: Option<String>,
    file_path_absolute: Option<String>,
    prj_path: PathBuf,
    pub file_selected_idx: Option<usize>,
    pub connection_data: ConnectionData,
    pub ssh_cfg: Option<SshCfg>,
    pub opened_folder: Option<String>,
    pub export_folder: Option<String>,
    pub is_loading_screen_active: Option<bool>,
    pub is_file_list_empty: Option<bool>,
}
impl MetaData {
    pub fn new(
        file_path_absolute: Option<String>,
        prj_path: PathBuf,
        file_selected_idx: Option<usize>,
        connection_data: ConnectionData,
        ssh_cfg: Option<SshCfg>,
        opened_folder: Option<String>,
        export_folder: Option<String>,
        is_loading_screen_active: Option<bool>,
        is_file_list_empty: Option<bool>,
    ) -> Self {
        let file_path_relative = file_path_absolute
            .clone()
            .map(|fpa| tf_to_annomap_key(fpa.clone(), Some(&prj_path)));
        MetaData {
            file_path_absolute,
            file_path_relative,
            prj_path,
            file_selected_idx,
            connection_data,
            ssh_cfg,
            opened_folder,
            export_folder,
            is_loading_screen_active,
            is_file_list_empty,
        }
    }
    pub fn from_filepath(
        file_path_absolute: String,
        file_selected_idx: usize,
        prj_path: PathBuf,
    ) -> Self {
        let file_path_relative = tf_to_annomap_key(file_path_absolute.clone(), Some(&prj_path));
        MetaData {
            file_path_absolute: Some(file_path_absolute),
            file_path_relative: Some(file_path_relative),
            prj_path: prj_path,
            file_selected_idx: Some(file_selected_idx),
            connection_data: ConnectionData::None,
            ssh_cfg: None,
            opened_folder: None,
            export_folder: None,
            is_loading_screen_active: None,
            is_file_list_empty: None,
        }
    }
    pub fn file_path_absolute(&self) -> Option<&str> {
        self.file_path_absolute.as_deref()
    }
    pub fn file_path_relative(&self) -> Option<&str> {
        self.file_path_relative.as_deref()
    }
}
