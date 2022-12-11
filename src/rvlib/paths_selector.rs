use std::path::Path;

use crate::{file_util, result::RvResult, rverr};

fn list_file_labels(
    file_paths: &[String],
    mut filter_predicate: impl FnMut(&str) -> bool,
) -> RvResult<Vec<(usize, String)>> {
    file_paths
        .iter()
        .enumerate()
        .filter(|(_, p)| filter_predicate(p))
        .map(|(i, p)| Ok((i, file_util::to_name_str(Path::new(p))?.to_string())))
        .collect::<RvResult<Vec<_>>>()
}

fn make_folder_label(folder_path: Option<&str>) -> RvResult<String> {
    match folder_path {
        Some(fp) => {
            let folder_path = Path::new(fp);
            let last = folder_path.ancestors().next();
            let one_before_last = folder_path.ancestors().nth(1);
            match (one_before_last, last) {
                (Some(obl), Some(last)) => Ok(if obl.to_string_lossy().is_empty() {
                    file_util::to_stem_str(last)?.to_string()
                } else {
                    format!(
                        "{}/{}",
                        file_util::to_stem_str(obl)?,
                        file_util::to_stem_str(last)?,
                    )
                }),
                (None, Some(l)) => Ok(if fp.is_empty() {
                    "".to_string()
                } else {
                    file_util::to_stem_str(l)?.to_string()
                }),
                _ => Err(rverr!("could not convert path {:?} to str", fp)),
            }
        }
        None => Ok("no folder selected".to_string()),
    }
}

pub struct PathsSelector {
    file_paths: Vec<String>,
    filtered_file_labels: Vec<(usize, String)>, // index-string pairs necessary due to filtering
    folder_label: String,
}

impl PathsSelector {
    fn label_idx_2_path_idx(&self, label_idx: usize) -> usize {
        self.filtered_file_labels[label_idx].0
    }

    pub fn new(mut file_paths: Vec<String>, folder_path: Option<String>) -> RvResult<Self> {
        file_paths.sort();
        let filtered_file_labels = list_file_labels(&file_paths, |_| true)?;
        let folder_label = make_folder_label(folder_path.as_deref())?;
        Ok(PathsSelector {
            file_paths,
            filtered_file_labels,
            folder_label,
        })
    }

    pub fn file_selected_path(&self, filtered_label_idx: usize) -> &str {
        self.file_paths[self.label_idx_2_path_idx(filtered_label_idx)].as_str()
    }

    pub fn filter(&mut self, filter_predicate: impl FnMut(&str) -> bool) -> RvResult<()> {
        self.filtered_file_labels = list_file_labels(&self.file_paths, filter_predicate)?;
        Ok(())
    }

    pub fn filtered_idx_file_label_pairs(&self) -> &Vec<(usize, String)> {
        &self.filtered_file_labels
    }

    pub fn filtered_file_paths(&self) -> Vec<&str> {
        self.filtered_file_labels
            .iter()
            .map(|(idx, _)| self.file_paths[*idx].as_str())
            .collect()
    }

    pub fn folder_label(&self) -> &str {
        self.folder_label.as_str()
    }

    pub fn idx_of_file_label(&self, file_label: &str) -> Option<usize> {
        self.filtered_file_labels
            .iter()
            .enumerate()
            .find(|(_, (_, fl))| fl == file_label)
            .map(|(idx, _)| idx)
    }
}
