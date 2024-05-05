use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{
    cfg::{AzureBlobCfg, PyHttpReaderCfg, SshCfg},
    file_util::tf_to_annomap_key,
};

#[derive(Clone, Default, PartialEq, Eq)]
pub struct FilePathPair {
    file_path_absolute: String,
    file_path_relative: String,
}
impl FilePathPair {
    pub fn new(file_path_absolute: String, prj_path: &Path) -> Self {
        let file_path_relative = tf_to_annomap_key(file_path_absolute.clone(), Some(prj_path));
        FilePathPair {
            file_path_absolute,
            file_path_relative,
        }
    }
    pub fn file_path_absolute(&self) -> &str {
        &self.file_path_absolute
    }
    pub fn file_path_relative(&self) -> &str {
        &self.file_path_relative
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct MetaDataFlags {
    pub is_loading_screen_active: Option<bool>,
    pub is_file_list_empty: Option<bool>,
}

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
    file_path_pair: Option<FilePathPair>,
    pub file_selected_idx: Option<usize>,
    pub connection_data: ConnectionData,
    pub ssh_cfg: Option<SshCfg>,
    pub opened_folder: Option<String>,
    pub export_folder: Option<String>,
    pub flags: MetaDataFlags,
}
impl MetaData {
    pub fn new(
        file_path_pair: Option<FilePathPair>,
        file_selected_idx: Option<usize>,
        connection_data: ConnectionData,
        ssh_cfg: Option<SshCfg>,
        opened_folder: Option<String>,
        export_folder: Option<String>,
        flags: MetaDataFlags,
    ) -> Self {
        MetaData {
            file_path_pair,
            file_selected_idx,
            connection_data,
            ssh_cfg,
            opened_folder,
            export_folder,
            flags
        }
    }
    pub fn from_filepath(
        file_path_absolute: String,
        file_selected_idx: usize,
        prj_path: &Path,
    ) -> Self {
        MetaData {
            file_path_pair: Some(FilePathPair::new(file_path_absolute, prj_path)),
            file_selected_idx: Some(file_selected_idx),
            connection_data: ConnectionData::None,
            ssh_cfg: None,
            opened_folder: None,
            export_folder: None,
            flags: MetaDataFlags::default(),
        }
    }
    pub fn file_path_absolute(&self) -> Option<&str> {
        self.file_path_pair
            .as_ref()
            .map(|fpp| fpp.file_path_absolute())
    }
    pub fn file_path_relative(&self) -> Option<&str> {
        self.file_path_pair
            .as_ref()
            .map(|fpp| fpp.file_path_relative())
    }
}
