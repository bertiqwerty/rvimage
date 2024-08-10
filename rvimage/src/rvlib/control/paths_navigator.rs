use super::{filter::FilterExpr, SortType};
use crate::{
    file_util::PathPair,
    paths_selector::PathsSelector,
    result::{ignore_error, trace_ok_err},
    world::ToolsDataMap,
};
use exmex::prelude::*;
use rvimage_domain::RvResult;

fn next(file_selected_idx: usize, files_len: usize) -> usize {
    if file_selected_idx < files_len - 1 {
        file_selected_idx + 1
    } else {
        files_len - 1
    }
}

fn prev(file_selected_idx: usize, files_len: usize) -> usize {
    if file_selected_idx >= files_len {
        files_len - 1
    } else if file_selected_idx > 0 {
        file_selected_idx - 1
    } else {
        0
    }
}
#[derive(Default)]
pub struct PathsNavigator {
    file_label_selected_idx: Option<usize>,
    paths_selector: Option<PathsSelector>,
    scroll_to_selected_label: bool,
}
impl PathsNavigator {
    pub fn new(mut paths_selector: Option<PathsSelector>, sort_type: SortType) -> RvResult<Self> {
        if let Some(ps) = &mut paths_selector {
            match sort_type {
                SortType::Natural => ps.natural_sort()?,
                SortType::Alphabetical => ps.alphabetical_sort()?,
            }
        };
        Ok(Self {
            file_label_selected_idx: None,
            paths_selector,
            scroll_to_selected_label: false,
        })
    }
    fn pn(&mut self, f: fn(usize, usize) -> usize) {
        if let Some(idx) = self.file_label_selected_idx {
            if let Some(ps) = &self.paths_selector {
                self.file_label_selected_idx = Some(f(idx, ps.len_filtered()));
                self.scroll_to_selected_label = true;
            }
        }
    }
    pub fn next(&mut self) {
        self.pn(next);
    }
    pub fn prev(&mut self) {
        self.pn(prev);
    }
    pub fn file_label_selected_idx(&self) -> Option<usize> {
        self.file_label_selected_idx
    }
    pub fn len_filtered(&self) -> Option<usize> {
        self.paths_selector.as_ref().map(|ps| ps.len_filtered())
    }
    pub fn scroll_to_selected_label(&self) -> bool {
        self.scroll_to_selected_label
    }
    pub fn activate_scroll_to_selected_label(&mut self) {
        self.scroll_to_selected_label = true;
    }
    pub fn deactivate_scroll_to_selected_label(&mut self) {
        self.scroll_to_selected_label = false;
    }
    /// makes sure the idx actually exists
    pub fn select_label_idx(&mut self, filtered_label_idx: Option<usize>) {
        if let (Some(idx), Some(ps)) = (filtered_label_idx, self.paths_selector()) {
            if idx < ps.len_filtered() {
                self.file_label_selected_idx = Some(idx);
            }
        }
    }

    fn idx_of_file_label(&self, file_label: &str) -> Option<usize> {
        match self.paths_selector() {
            Some(ps) => ps.idx_of_file_label(file_label),
            None => None,
        }
    }

    pub fn select_file_label(&mut self, file_label: &str) {
        self.select_label_idx(self.idx_of_file_label(file_label));
    }

    pub fn paths_selector(&self) -> &Option<PathsSelector> {
        &self.paths_selector
    }

    pub fn natural_sort(
        &mut self,
        filter_str: &str,
        tools_data_map: &ToolsDataMap,
        active_tool_name: Option<&str>,
    ) -> RvResult<()> {
        if let Some(ps) = &mut self.paths_selector {
            ps.natural_sort()?;
            self.filter(filter_str, tools_data_map, active_tool_name)?;
        }
        Ok(())
    }
    pub fn alphabetical_sort(
        &mut self,
        filter_str: &str,
        tools_data_map: &ToolsDataMap,
        active_tool_name: Option<&str>,
    ) -> RvResult<()> {
        if let Some(ps) = &mut self.paths_selector {
            ps.alphabetical_sort()?;
            self.filter(filter_str, tools_data_map, active_tool_name)?;
        }
        Ok(())
    }

    fn filter_by_pred(&mut self, filter_predicate: impl FnMut(&str) -> bool) -> RvResult<()> {
        if let Some(ps) = &mut self.paths_selector {
            let unfiltered_idx_before_filter =
                if let Some(filtered_idx) = self.file_label_selected_idx {
                    self.scroll_to_selected_label = true;
                    let (unfiltered_idx, _) = ps.filtered_idx_file_label_pairs(filtered_idx);
                    Some(unfiltered_idx)
                } else {
                    None
                };
            ps.filter(filter_predicate)?;
            self.file_label_selected_idx = match unfiltered_idx_before_filter {
                Some(unfiltered_idx) => ps
                    .filtered_iter()
                    .enumerate()
                    .find(|(_, (uidx, _))| *uidx == unfiltered_idx)
                    .map(|(fidx, _)| fidx),
                None => None,
            };
        }
        Ok(())
    }

    pub fn filter(
        &mut self,
        s: &str,
        tools_data_map: &ToolsDataMap,
        active_tool_name: Option<&str>,
    ) -> RvResult<()> {
        if let Some(filter_pred) =
            ignore_error(FilterExpr::parse(s).and_then(|expr| expr.eval(&[])))
        {
            let filter_pred_wrapper = |path: &str| {
                trace_ok_err(filter_pred.apply(path, Some(tools_data_map), active_tool_name))
                    .unwrap_or(true)
            };
            self.filter_by_pred(filter_pred_wrapper)?;
        } else {
            let trimmed = s.trim();
            let filter_pred = |path: &str| {
                if path.is_empty() {
                    true
                } else {
                    path.contains(trimmed)
                }
            };
            self.filter_by_pred(filter_pred)?;
        }
        Ok(())
    }

    pub fn folder_label(&self) -> Option<&str> {
        self.paths_selector().as_ref().map(|ps| ps.folder_label())
    }

    pub fn file_path(&self, file_idx: usize) -> Option<&PathPair> {
        self.paths_selector()
            .as_ref()
            .and_then(|ps| ps.file_selected_path(file_idx))
    }
}

#[test]
fn test_prev_next() {
    assert_eq!(next(3, 4), 3);
    assert_eq!(next(2, 4), 3);
    assert_eq!(next(5, 4), 3);
    assert_eq!(next(1, 4), 2);
    assert_eq!(prev(3, 4), 2);
    assert_eq!(prev(2, 3), 1);
    assert_eq!(prev(3, 3), 2);
    assert_eq!(prev(4, 3), 2);
    assert_eq!(prev(9, 3), 2);
}
