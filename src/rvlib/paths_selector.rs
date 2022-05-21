use std::path::Path;

use crate::{
    format_rverr,
    result::{RvError, RvResult},
    util,
};

pub fn to_stem_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(p.file_stem())
        .map_err(|e| format_rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}

pub fn to_name_str(p: &Path) -> RvResult<&str> {
    util::osstr_to_str(p.file_name())
        .map_err(|e| format_rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}
fn list_file_labels(file_paths: &[String], filter_str: &str) -> RvResult<Vec<(usize, String)>> {
    file_paths
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            if filter_str.is_empty() {
                true
            } else {
                p.contains(filter_str)
            }
        })
        .map(|(i, p)| Ok((i, to_name_str(Path::new(p))?.to_string())))
        .collect::<RvResult<Vec<_>>>()
}

fn make_folder_label(folder_path: Option<&str>) -> RvResult<String> {
    match folder_path {
        Some(sf) => {
            let folder_path = Path::new(sf);
            let last = folder_path.ancestors().next();
            let one_before_last = folder_path.ancestors().nth(1);
            match (one_before_last, last) {
                (Some(obl), Some(l)) => Ok(format!("{}/{}", to_stem_str(obl)?, to_stem_str(l)?,)),
                (None, Some(l)) => Ok(to_stem_str(l)?.to_string()),
                _ => Err(format_rverr!("could not convert path {:?} to str", sf)),
            }
        }
        None => Ok("no folder selected".to_string()),
    }
}

pub struct PathsSelector {
    file_paths: Vec<String>,
    filtered_file_labels: Vec<(usize, String)>,
    folder_label: String,
}
impl PathsSelector {
    fn label_idx_2_path_idx(&self, label_idx: usize) -> usize {
        self.filtered_file_labels[label_idx].0
    }
    pub fn new(mut file_paths: Vec<String>, folder_path: Option<String>) -> RvResult<Self> {
        optick::event!();
        file_paths.sort();
        let filtered_file_labels = list_file_labels(&file_paths, "")?;
        let folder_label = make_folder_label(folder_path.as_deref())?;
        Ok(PathsSelector {
            file_paths,
            filtered_file_labels,
            folder_label,
        })
    }
    pub fn file_selected_path(&self, label_selected_idx: usize) -> &str {
        self.file_paths[self.label_idx_2_path_idx(label_selected_idx)].as_str()
    }
    
    pub fn filter(&mut self, filter_str: &str) -> RvResult<()> {
        optick::event!();
        self.filtered_file_labels = list_file_labels(&self.file_paths, filter_str)?;
        Ok(())
    }
    pub fn file_labels(&self) -> &Vec<(usize, String)> {
        &self.filtered_file_labels
    }
    pub fn filtered_file_paths(&self) -> Vec<String> {
        optick::event!();
        self.filtered_file_labels
            .iter()
            .map(|(idx, _)| self.file_paths[*idx].clone())
            .collect()
    }
    pub fn folder_label(&self) -> &str {
        self.folder_label.as_str()
    }
}
