use super::{
    annotations::{BrushAnnotations, ClipboardData},
    core::{self, AnnotationsMap, LabelInfo},
};
use crate::{
    cfg::ExportPath,
    domain::{Canvas, ShapeI},
    result::RvResult,
    rverr, BrushLine,
};
use crate::{domain::TPtF, implement_annotations_getters};

use serde::{Deserialize, Serialize};

pub type BrushAnnoMap = AnnotationsMap<Canvas>;

pub const MAX_THICKNESS: f64 = 300.0;
pub const MIN_THICKNESS: f64 = 1.0;
pub const MAX_INTENSITY: f64 = 1.0;
pub const MIN_INTENSITY: f64 = 0.01;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub thickness: TPtF,
    pub intensity: TPtF,
    #[serde(skip)]
    pub is_selection_change_needed: bool,
    #[serde(skip)]
    pub core_options: core::Options,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            thickness: 15.0,
            intensity: 0.5,
            is_selection_change_needed: false,
            core_options: core::Options::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct BrushToolData {
    pub annotations_map: BrushAnnoMap,
    // we might want to show this while it is being drawn,
    // (line, cat_idx)
    pub tmp_line: Option<(BrushLine, usize)>,
    pub options: Options,
    pub label_info: LabelInfo,
    #[serde(skip)]
    pub clipboard: Option<ClipboardData<Canvas>>,
    pub export_folder: ExportPath,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
    pub fn set_annotations_map(&mut self, map: BrushAnnoMap) -> RvResult<()> {
        for (_, (annos, _)) in map.iter() {
            for cat_idx in annos.cat_idxs() {
                let len = self.label_info.len();
                if *cat_idx >= len {
                    return Err(rverr!(
                        "cat idx {cat_idx} does not have a label, out of bounds, {len}"
                    ));
                }
            }
        }
        self.annotations_map = map;
        Ok(())
    }
}
impl Eq for BrushToolData {}
