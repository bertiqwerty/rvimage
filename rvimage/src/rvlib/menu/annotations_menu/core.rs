use egui::Ui;
use rvimage_domain::{RvResult, rverr};

use crate::{
    InstanceAnnotate, ToolsDataMap,
    file_util::PathPair,
    tools::{ATTRIBUTES_NAME, BBOX_NAME, BRUSH_NAME},
    tools_data::{AnnotationsMap, ToolSpecifics, annotations::InstanceAnnotations},
};

#[derive(Clone, Copy, Default)]
pub enum FilterRelation {
    // files that are contained in the list of filtered files
    #[default]
    Available,
    // files that are NOT contained the list of filtered files
    Missing,
}
impl FilterRelation {
    pub fn apply<'a>(
        &'a self,
        mut filtered_filepaths: impl Iterator<Item = &'a &'a PathPair>,
        path_tdm_key: &'a str,
    ) -> bool {
        let is_key_in_filtered_paths =
            filtered_filepaths.any(|fp| fp.path_relative() == path_tdm_key);
        match self {
            Self::Available => is_key_in_filtered_paths,
            Self::Missing => !is_key_in_filtered_paths,
        }
    }
    pub fn select<T>(&self, option_available: T, option_missing: T) -> T {
        match self {
            Self::Available => option_available,
            Self::Missing => option_missing,
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq)]
pub struct ToolChoice {
    pub brush: bool,
    pub bbox: bool,
    pub attributes: bool,
}
impl ToolChoice {
    pub fn ui(&mut self, ui: &mut Ui, skip_attributes: bool) {
        ui.label("Select tool who's annotations you are interested in");
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.bbox, BBOX_NAME);
            ui.checkbox(&mut self.brush, BRUSH_NAME);
            if !skip_attributes {
                ui.checkbox(&mut self.attributes, ATTRIBUTES_NAME)
                    .on_hover_text("only for propagation");
            }
        });
    }
    pub fn run_mut(
        &self,
        ui: &mut Ui,
        tdm: &mut ToolsDataMap,
        mut f_bbox: impl FnMut(&mut Ui, &mut ToolsDataMap) -> RvResult<()>,
        mut f_brush: impl FnMut(&mut Ui, &mut ToolsDataMap) -> RvResult<()>,
        mut f_attr: impl FnMut(&mut Ui, &mut ToolsDataMap) -> RvResult<()>,
    ) -> RvResult<()> {
        if self.bbox {
            f_bbox(ui, tdm)?;
        }
        if self.brush {
            f_brush(ui, tdm)?;
        }
        if self.attributes {
            f_attr(ui, tdm)?;
        }
        Ok(())
    }
    pub fn run<'a>(
        tool_name: &'static str,
        tdm: &'a ToolsDataMap,
        mut f_bbox: impl FnMut(&'a ToolsDataMap) -> RvResult<()>,
        mut f_brush: impl FnMut(&'a ToolsDataMap) -> RvResult<()>,
    ) -> RvResult<()> {
        match tool_name {
            BBOX_NAME => f_bbox(tdm),
            BRUSH_NAME => f_brush(tdm),
            _ => Err(rverr!("cannot run. unknown tool {tool_name}")),
        }
    }

    pub fn is_some(&self, skip_attributes: bool) -> bool {
        self.bbox || self.brush || (!skip_attributes && self.attributes)
    }
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

/// Returns an iterator over file idx, filename, toolname, number of annotations in file
pub(super) fn iter_files_of_instance_tool<'a, T>(
    tdm: &'a ToolsDataMap,
    filepaths: &'a [(usize, &PathPair)],
    tool_name: &'static str,
    unwrap_specifics: impl Fn(&ToolSpecifics) -> RvResult<&AnnotationsMap<T>>,
    filter_relation: FilterRelation,
    relevant_catidxs: Option<&'a [usize]>,
) -> RvResult<impl Iterator<Item = (Option<usize>, &'a str, &'static str, usize)> + 'a>
where
    T: InstanceAnnotate + 'a,
{
    if tdm.contains_key(tool_name) {
        let datamap = unwrap_specifics(&tdm[tool_name].specifics)?;

        let iter_available =
            filepaths
                .iter()
                .filter_map(move |(idx, filepath)| match filter_relation {
                    FilterRelation::Available => {
                        let annos = datamap.get(filepath.path_relative());
                        annos.map(|(annos, _)| {
                            let n_annos = count_annos(annos, relevant_catidxs);
                            (Some(*idx), filepath.path_relative(), tool_name, n_annos)
                        })
                    }
                    FilterRelation::Missing => None,
                });
        let iter_missing = datamap
            .iter()
            .filter(move |(k, _)| {
                matches!(filter_relation, FilterRelation::Missing)
                    && filter_relation.apply(filepaths.iter().map(|(_, fp)| fp), k)
            })
            .map(move |(k, (annos, _))| (None, k.as_str(), tool_name, annos.len()));
        Ok(iter_available.chain(iter_missing))
    } else {
        Err(rverr!("Tool {tool_name} has no data"))
    }
}
