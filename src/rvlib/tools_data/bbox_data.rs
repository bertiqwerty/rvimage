use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{
    annotations::{BboxAnnotations, ClipboardData},
    core::{LabelInfo, OUTLINE_THICKNESS_CONVERSION},
};
use crate::{
    cfg::{get_cfg, ExportPath},
    domain::Shape,
    file_util, implement_annotations_getters,
    result::RvResult,
    rverr,
    tools_data::{annotations::SplitMode, core},
    GeoFig,
};

/// filename -> (annotations per file, file dimensions)
pub type AnnotationsMap = HashMap<String, (BboxAnnotations, Shape)>;

#[derive(Clone, Copy, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct Options {
    pub core_options: core::Options,
    pub auto_paste: bool,
    pub is_anno_rm_triggered: bool,
    pub is_coco_import_triggered: bool,
    pub split_mode: SplitMode,
    pub export_absolute: bool,
    pub fill_alpha: u8,
    pub outline_alpha: u8,
    pub outline_thickness: u16,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            core_options: core::Options::default(),
            auto_paste: false,
            is_anno_rm_triggered: false,
            is_coco_import_triggered: false,
            split_mode: SplitMode::default(),
            export_absolute: false,
            fill_alpha: 30,
            outline_alpha: 255,
            outline_thickness: OUTLINE_THICKNESS_CONVERSION as u16,
        }
    }
}
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct BboxSpecificData {
    pub label_info: LabelInfo,
    pub annotations_map: AnnotationsMap,
    pub clipboard: Option<ClipboardData<GeoFig>>,
    pub options: Options,
    pub coco_file: ExportPath,
}

impl BboxSpecificData {
    implement_annotations_getters!(BboxAnnotations);

    fn separate_data(self) -> (LabelInfo, AnnotationsMap, ExportPath) {
        (self.label_info, self.annotations_map, self.coco_file)
    }

    pub fn n_annotated_images(&self, paths: &[&str]) -> usize {
        paths
            .iter()
            .filter(|p| {
                if let Some((anno, _)) = self.annotations_map.get(**p) {
                    !anno.elts().is_empty()
                } else {
                    false
                }
            })
            .count()
    }

    pub fn from_bbox_export_data(input_data: BboxExportData) -> RvResult<Self> {
        let label_info = LabelInfo::from_iter(
            input_data
                .labels
                .into_iter()
                .zip(input_data.colors.into_iter())
                .zip(input_data.cat_ids.into_iter()),
        )?;
        let mut out_data = Self {
            label_info,
            annotations_map: HashMap::new(),
            clipboard: None,
            options: Options {
                core_options: core::Options {
                    visible: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            coco_file: input_data.coco_file,
        };
        out_data.set_annotations_map(
            input_data
                .annotations
                .into_iter()
                .map(|(s, (geos, cat_ids, dims))| {
                    (s, (BboxAnnotations::from_elts_cats(geos, cat_ids), dims))
                })
                .collect(),
        )?;
        Ok(out_data)
    }

    pub fn retain_fileannos_in_folder(&mut self, folder: &str) {
        self.annotations_map
            .retain(|f, _| file_util::url_encode(f).starts_with(folder));
    }

    pub fn new() -> Self {
        let label_info = LabelInfo::default();
        let cfg = get_cfg().expect("could not read config nor create default config");

        BboxSpecificData {
            label_info,
            annotations_map: HashMap::new(),
            clipboard: None,
            options: Options {
                core_options: core::Options {
                    visible: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            coco_file: if let Some(cf) = cfg.coco_file {
                cf
            } else {
                ExportPath::default()
            },
        }
    }

    pub fn set_annotations_map(&mut self, map: AnnotationsMap) -> RvResult<()> {
        for (_, (annos, _)) in map.iter() {
            for cat_idx in annos.cat_idxs() {
                let len = self.label_info.len();
                if *cat_idx >= len {
                    return Err(rverr!(
                        "cat idx {} does not have a label, out of bounds, {}",
                        cat_idx,
                        len
                    ));
                }
            }
        }
        self.annotations_map = map;
        Ok(())
    }
}

impl Default for BboxSpecificData {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct BboxExportData {
    pub labels: Vec<String>,
    pub colors: Vec<[u8; 3]>,
    pub cat_ids: Vec<u32>,
    // filename, bounding boxes, classes of the boxes, dimensions of the image
    pub annotations: HashMap<String, (Vec<GeoFig>, Vec<usize>, Shape)>,
    pub coco_file: ExportPath,
    pub is_export_absolute: bool,
}

impl BboxExportData {
    pub fn from_bbox_data(bbox_specifics: BboxSpecificData) -> Self {
        let is_export_absolute = bbox_specifics.options.export_absolute;
        let (label_info, annotations_map, coco_file) = bbox_specifics.separate_data();
        let annotations = annotations_map
            .into_iter()
            .map(|(filename, (annos, shape))| {
                let (bbs, labels, _) = annos.separate_data();
                (filename, (bbs, labels, shape))
            })
            .collect::<HashMap<_, _>>();
        let (labels, colors, cat_ids) = label_info.separate_data();
        BboxExportData {
            labels,
            colors,
            cat_ids,
            annotations,
            coco_file,
            is_export_absolute,
        }
    }
}
