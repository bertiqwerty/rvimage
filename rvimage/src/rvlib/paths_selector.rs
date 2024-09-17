use std::{cmp::Ordering, path::Path};

use rvimage_domain::{rverr, RvResult};

use crate::{
    control::SortType,
    file_util::{self, PathPair},
    util::natural_cmp,
};

fn list_file_labels(
    file_paths: &[PathPair],
    mut filter_predicate: impl FnMut(&str) -> bool,
) -> RvResult<Vec<(usize, String)>> {
    file_paths
        .iter()
        .enumerate()
        .filter(|(_, p)| filter_predicate(p.path_relative()))
        .map(|(i, p)| {
            Ok((
                i,
                file_util::to_name_str(Path::new(p.path_relative()))?.to_string(),
            ))
        })
        .collect::<RvResult<Vec<_>>>()
}

fn make_folder_label(folder_path: Option<&str>) -> RvResult<String> {
    match folder_path {
        Some(fp) => {
            let folder_path = Path::new(fp);
            if folder_path.is_file() {
                return Err(rverr!("path {folder_path:?} is a file"));
            }
            let last = folder_path.ancestors().next();
            let one_before_last = folder_path.ancestors().nth(1);
            match (one_before_last, last) {
                (Some(obl), Some(last)) => Ok(if obl.to_string_lossy().is_empty() {
                    file_util::to_name_str(last)?.to_string()
                } else {
                    format!(
                        "{}/{}",
                        file_util::to_name_str(obl)?,
                        file_util::to_name_str(last)?,
                    )
                }),
                (None, Some(last)) => Ok(if fp.is_empty() {
                    "".to_string()
                } else {
                    file_util::to_name_str(last)?.to_string()
                }),
                _ => Err(rverr!("could not convert path {:?} to str", fp)),
            }
        }
        None => Ok("no folder selected".to_string()),
    }
}

pub struct PathsSelector {
    file_paths: Vec<PathPair>,
    filtered_file_labels: Vec<(usize, String)>, // index-string pairs necessary due to filtering
    folder_label: String,
}

impl PathsSelector {
    fn label_idx_2_path_idx(&self, label_idx: usize) -> Option<usize> {
        if label_idx >= self.filtered_file_labels.len() {
            None
        } else {
            Some(self.filtered_file_labels[label_idx].0)
        }
    }

    fn sort(&mut self, sort_by_filename: bool, f_cmp: fn(&str, &str) -> Ordering) -> RvResult<()> {
        self.file_paths.sort_by(|s1, s2| {
            if sort_by_filename {
                match (s1.filename(), s2.filename()) {
                    (Ok(fname1), Ok(fname2)) => f_cmp(fname1, fname2),
                    _ => {
                        tracing::warn!("could not sort {s1:?} and {s2:?} by filename");
                        f_cmp(s1.path_relative(), s2.path_relative())
                    }
                }
            } else {
                f_cmp(s1.path_relative(), s2.path_relative())
            }
        });
        self.filtered_file_labels = list_file_labels(&self.file_paths, |_| true)?;
        Ok(())
    }

    pub fn natural_sort(&mut self, sort_by_filename: bool) -> RvResult<()> {
        self.sort(sort_by_filename, natural_cmp)
    }

    pub fn alphabetical_sort(&mut self, sort_by_filename: bool) -> RvResult<()> {
        fn f_cmp(s1: &str, s2: &str) -> Ordering {
            s1.cmp(s2)
        }
        self.sort(sort_by_filename, f_cmp)
    }

    pub fn new(mut file_paths: Vec<PathPair>, folder_path: Option<String>) -> RvResult<Self> {
        match SortType::default() {
            SortType::Natural => {
                file_paths.sort_by(|s1, s2| natural_cmp(s1.path_relative(), s2.path_relative()))
            }
            SortType::Alphabetical => {
                file_paths.sort_by(|s1, s2| s1.path_relative().cmp(s2.path_relative()))
            }
        }
        let filtered_file_labels = list_file_labels(&file_paths, |_| true)?;
        let folder_label = make_folder_label(folder_path.as_deref())?;
        Ok(PathsSelector {
            file_paths,
            filtered_file_labels,
            folder_label,
        })
    }

    pub fn file_selected_path(&self, filtered_label_idx: usize) -> Option<&PathPair> {
        let idx = self.label_idx_2_path_idx(filtered_label_idx);
        idx.map(|idx| &self.file_paths[idx])
    }

    pub fn filter(&mut self, filter_predicate: impl FnMut(&str) -> bool) -> RvResult<()> {
        self.filtered_file_labels = list_file_labels(&self.file_paths, filter_predicate)?;
        Ok(())
    }

    pub fn filtered_idx_file_label_pairs(&self, idx: usize) -> (usize, &str) {
        (
            self.filtered_file_labels[idx].0,
            &self.filtered_file_labels[idx].1,
        )
    }
    pub fn len_filtered(&self) -> usize {
        self.filtered_file_labels.len()
    }
    pub fn filtered_iter(&self) -> impl Iterator<Item = (usize, &str)> {
        self.filtered_file_labels
            .iter()
            .map(|(idx, fl)| (*idx, fl.as_str()))
    }

    pub fn filtered_file_paths(&self) -> Vec<&PathPair> {
        self.filtered_file_labels
            .iter()
            .map(|(idx, _)| &self.file_paths[*idx])
            .collect()
    }
    pub fn filtered_abs_file_paths(&self) -> Vec<&str> {
        self.filtered_file_labels
            .iter()
            .map(|(idx, _)| self.file_paths[*idx].path_absolute())
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
