use std::collections::HashMap;

use rvimage_domain::{PtF, RvResult};

use crate::{
    file_util::PathPair,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
};

use super::{
    annotations::InstanceAnnotations,
    attributes_data::{AttrMap, AttrVal},
    AccessInstanceData, AttributesToolData, BboxToolData, BrushToolData, InstanceAnnotate,
};

fn iter_attributes_of_files<'a>(
    atd: &'a AttributesToolData,
    filepaths: &'a [(usize, &PathPair)],
) -> impl Iterator<Item = (usize, &'a AttrMap)> + 'a {
    atd.anno_iter()
        .filter_map(move |(anno_key_filename, (attrmap, _))| {
            if let Some((idx, _)) = filepaths
                .iter()
                .find(|(_, fp)| fp.path_relative() == anno_key_filename)
            {
                Some((*idx, attrmap))
            } else {
                None
            }
        })
}
fn plot_instance_anno_counts<T, D>(
    data: &D,
    selected: &HashMap<String, bool>,
    filepaths: &[(usize, &PathPair)],
) -> RvResult<Vec<PtF>>
where
    T: InstanceAnnotate,
    D: AccessInstanceData<T>,
{
    let relevant_indices = selected
        .iter()
        .filter(|(_, is_selected)| **is_selected)
        .flat_map(|(selected_label, _)| {
            data.label_info()
                .labels()
                .iter()
                .position(|label| label == selected_label)
        })
        .collect::<Vec<_>>();
    let mut plot = vec![];
    for (fidx, _, count) in iter_files_of_instance_tool(
        data,
        filepaths,
        if relevant_indices.is_empty() {
            None
        } else {
            Some(&relevant_indices)
        },
    )? {
        if let Some(fidx) = fidx {
            plot.push(PtF {
                x: fidx as f64,
                y: count as f64,
            });
        }
    }
    Ok(plot)
}
fn count_annos<'a, T>(
    annos: &'a InstanceAnnotations<T>,
    relevant_catidxs: Option<&'a [usize]>,
) -> usize
where
    T: InstanceAnnotate,
    T: PartialEq,
    T: std::default::Default,
{
    if let Some(relevant_catidxs) = relevant_catidxs {
        annos
            .iter()
            .filter(|(_, cat_idx, _)| relevant_catidxs.contains(cat_idx))
            .count()
    } else {
        annos.len()
    }
}
pub fn iter_files_of_instance_tool<'a, T, L>(
    data: &'a L,
    filepaths: &'a [(usize, &PathPair)],
    relevant_catidxs: Option<&'a [usize]>,
) -> RvResult<impl Iterator<Item = (Option<usize>, &'a str, usize)> + 'a>
where
    T: InstanceAnnotate + 'a,
    L: AccessInstanceData<T>,
{
    let datamap = data.annotations_map();

    let iter_available = filepaths.iter().filter_map(move |(idx, filepath)| {
        let annos = datamap.get(filepath.path_relative());
        annos.map(|(annos, _)| {
            let n_annos = count_annos(annos, relevant_catidxs);
            (Some(*idx), filepath.path_relative(), n_annos)
        })
    });
    Ok(iter_available)
}
pub trait PlotAnnotationStats {
    /// Create a vector of x-y-coordinates to be plotted
    fn plot(
        &self,
        selected: &HashMap<String, bool>,
        filepaths: &[(usize, &PathPair)],
    ) -> RvResult<HashMap<String, Vec<PtF>>>;
}

impl PlotAnnotationStats for BboxToolData {
    fn plot(
        &self,
        selected: &HashMap<String, bool>,
        filepaths: &[(usize, &PathPair)],
    ) -> RvResult<HashMap<String, Vec<PtF>>> {
        let plt = plot_instance_anno_counts(self, selected, filepaths)?;
        Ok(HashMap::from([(BBOX_NAME.into(), plt)]))
    }
}
impl PlotAnnotationStats for BrushToolData {
    fn plot(
        &self,
        selected: &HashMap<String, bool>,
        filepaths: &[(usize, &PathPair)],
    ) -> RvResult<HashMap<String, Vec<PtF>>> {
        let plt = plot_instance_anno_counts(self, selected, filepaths)?;
        Ok(HashMap::from([(BRUSH_NAME.into(), plt)]))
    }
}
impl PlotAnnotationStats for AttributesToolData {
    fn plot(
        &self,
        selected: &HashMap<String, bool>,
        filepaths: &[(usize, &PathPair)],
    ) -> RvResult<HashMap<String, Vec<PtF>>> {
        let mut output_plots = HashMap::new();
        for (selected_attr, is_selected) in selected.iter() {
            if *is_selected {
                let mut plot = vec![];
                for (file_idx, attr_map) in iter_attributes_of_files(self, filepaths) {
                    let value = attr_map.get(selected_attr);
                    if let Some(value) = value {
                        let y = match value {
                            AttrVal::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                            AttrVal::Float(x) => *x,
                            AttrVal::Int(n) => n.map(|n| n as f64),
                            AttrVal::Str(s) => Some(s.len() as f64),
                        };
                        if let Some(y) = y {
                            plot.push(PtF {
                                x: file_idx as f64,
                                y,
                            });
                        }
                    }
                }
                if !plot.is_empty() {
                    output_plots.insert(format!("{ATTRIBUTES_NAME}_{selected_attr}"), plot);
                }
            }
        }
        Ok(output_plots)
    }
}
