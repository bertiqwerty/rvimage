use std::{collections::HashMap, mem};

use serde::{Deserialize, Serialize};

use super::annotations::BboxAnnotations;
use crate::{
    annotations::SplitMode,
    cfg::{get_cfg, CocoFile},
    domain::Shape,
    file_util, implement_annotations_getters,
    result::RvResult,
    rverr,
    util::true_indices,
    GeoFig,
};
const DEFAULT_LABEL: &str = "foreground";

/// filename -> (annotations per file, file dimensions)
pub type AnnotationsMap = HashMap<String, (BboxAnnotations, Shape)>;

fn color_dist(c1: [u8; 3], c2: [u8; 3]) -> f32 {
    let square_d = |i| (c1[i] as f32 - c2[i] as f32).powi(2);
    (square_d(0) + square_d(1) + square_d(2)).sqrt()
}

pub fn random_clr() -> [u8; 3] {
    let r = rand::random::<u8>();
    let g = rand::random::<u8>();
    let b = rand::random::<u8>();
    [r, g, b]
}

fn argmax_clr_dist(picklist: &[[u8; 3]], legacylist: &[[u8; 3]]) -> [u8; 3] {
    let (idx, _) = picklist
        .iter()
        .enumerate()
        .map(|(i, pickclr)| {
            let min_dist = legacylist
                .iter()
                .map(|legclr| color_dist(*legclr, *pickclr))
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0);
            (i, min_dist)
        })
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .unwrap();
    picklist[idx]
}

pub fn new_color(colors: &[[u8; 3]]) -> [u8; 3] {
    let mut new_clr_proposals = [[0u8, 0u8, 0u8]; 10];
    for new_clr in &mut new_clr_proposals {
        *new_clr = random_clr();
    }
    argmax_clr_dist(&new_clr_proposals, colors)
}

pub fn new_random_colors(n: usize) -> Vec<[u8; 3]> {
    let mut colors = vec![random_clr()];
    for _ in 0..(n - 1) {
        let color = new_color(&colors);
        colors.push(color);
    }
    colors
}
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct ClipboardData {
    geos: Vec<GeoFig>,
    cat_idxs: Vec<usize>,
}

impl ClipboardData {
    pub fn from_annotations(annos: &BboxAnnotations) -> Self {
        let selected_inds = true_indices(annos.selected_bbs());
        let bbs = selected_inds
            .clone()
            .map(|idx| annos.geos()[idx].clone())
            .collect();
        let cat_idxs = selected_inds.map(|idx| annos.cat_idxs()[idx]).collect();
        ClipboardData {
            geos: bbs,
            cat_idxs,
        }
    }

    pub fn geos(&self) -> &Vec<GeoFig> {
        &self.geos
    }

    pub fn cat_idxs(&self) -> &Vec<usize> {
        &self.cat_idxs
    }
}

#[derive(Clone, Copy, Deserialize, Serialize, Debug, PartialEq)]
pub struct Options {
    pub are_boxes_visible: bool,
    pub auto_paste: bool,
    pub is_anno_rm_triggered: bool,
    pub is_coco_import_triggered: bool,
    pub is_export_triggered: bool,
    pub is_colorchange_triggered: bool,
    pub is_redraw_annos_triggered: bool,
    pub split_mode: SplitMode,
    pub export_absolute: bool,
    pub fill_alpha: u8,
    pub outline_alpha: u8,
    pub outline_thickness: f32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            are_boxes_visible: true,
            auto_paste: false,
            is_anno_rm_triggered: false,
            is_coco_import_triggered: false,
            is_export_triggered: false,
            is_colorchange_triggered: false,
            is_redraw_annos_triggered: false,
            split_mode: SplitMode::default(),
            export_absolute: false,
            fill_alpha: 30,
            outline_alpha: 255,
            outline_thickness: 1.0,
        }
    }
}
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct BboxSpecificData {
    pub new_label: String,
    labels: Vec<String>,
    colors: Vec<[u8; 3]>,
    cat_ids: Vec<u32>,
    pub cat_idx_current: usize,
    annotations_map: AnnotationsMap,
    pub clipboard: Option<ClipboardData>,
    pub options: Options,
    pub coco_file: CocoFile,
}

impl BboxSpecificData {
    implement_annotations_getters!(BboxAnnotations);

    pub fn n_annotated_images(&self, paths: &[&str]) -> usize {
        paths
            .iter()
            .filter(|p| {
                if let Some((anno, _)) = self.annotations_map.get(**p) {
                    !anno.geos().is_empty()
                } else {
                    false
                }
            })
            .count()
    }

    pub fn from_bbox_export_data(input_data: BboxExportData) -> RvResult<Self> {
        let mut out_data = Self {
            new_label: DEFAULT_LABEL.to_string(),
            labels: vec![],
            colors: vec![],
            cat_ids: vec![],
            cat_idx_current: 0,
            annotations_map: HashMap::new(),
            clipboard: None,
            options: Options {
                are_boxes_visible: true,
                ..Default::default()
            },
            coco_file: input_data.coco_file,
        };
        for ((lab, clr), cat_id) in input_data
            .labels
            .into_iter()
            .zip(input_data.colors.into_iter())
            .zip(input_data.cat_ids.into_iter())
        {
            out_data.push(lab, Some(clr), Some(cat_id))?;
        }
        out_data.set_annotations_map(
            input_data
                .annotations
                .into_iter()
                .map(|(s, (bbs, cat_ids, dims))| {
                    (s, (BboxAnnotations::from_bbs_cats(bbs, cat_ids), dims))
                })
                .collect(),
        )?;
        Ok(out_data)
    }

    pub fn remove_catidx(&mut self, cat_idx: usize) {
        if self.labels.len() > 1 {
            self.labels.remove(cat_idx);
            self.colors.remove(cat_idx);
            self.cat_ids.remove(cat_idx);
            if self.cat_idx_current >= cat_idx.max(1) {
                self.cat_idx_current -= 1;
            }
            for (anno, _) in self.annotations_map.values_mut() {
                anno.reduce_cat_idxs(cat_idx);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.colors.len() == 0
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn find_default(&mut self) -> Option<&mut String> {
        self.labels.iter_mut().find(|lab| lab == &DEFAULT_LABEL)
    }

    pub fn push(
        &mut self,
        label: String,
        color: Option<[u8; 3]>,
        cat_id: Option<u32>,
    ) -> RvResult<()> {
        if self.labels.contains(&label) {
            Err(rverr!("label '{}' already exists", label))
        } else {
            self.labels.push(label);
            if let Some(clr) = color {
                if self.colors.contains(&clr) {
                    return Err(rverr!("color '{:?}' already exists", clr));
                }
                self.colors.push(clr);
            } else {
                let new_clr = new_color(&self.colors);
                self.colors.push(new_clr);
            }
            if let Some(cat_id) = cat_id {
                if self.cat_ids.contains(&cat_id) {
                    return Err(rverr!("cat id '{:?}' already exists", cat_id));
                }
                self.cat_ids.push(cat_id);
            } else if let Some(max_id) = self.cat_ids.iter().max() {
                self.cat_ids.push(max_id + 1);
            } else {
                self.cat_ids.push(1);
            }
            Ok(())
        }
    }

    pub fn retain_fileannos_in_folder(&mut self, folder: &str) {
        self.annotations_map
            .retain(|f, _| file_util::url_encode(f).starts_with(folder));
    }

    pub fn colors(&self) -> &Vec<[u8; 3]> {
        &self.colors
    }

    pub fn new_random_colors(&mut self) {
        self.colors = new_random_colors(self.colors.len());
    }

    pub fn labels(&self) -> &Vec<String> {
        &self.labels
    }

    pub fn cat_ids(&self) -> &Vec<u32> {
        &self.cat_ids
    }

    pub fn new() -> Self {
        let new_label = DEFAULT_LABEL.to_string();
        let new_color = [255, 255, 255];
        let labels = vec![new_label.clone()];
        let colors = vec![new_color];
        let cat_ids = vec![1];
        let cfg = get_cfg().expect("could not read config nor create default config");

        BboxSpecificData {
            new_label,
            labels,
            colors,
            cat_ids,
            cat_idx_current: 0,
            annotations_map: HashMap::new(),
            clipboard: None,
            options: Options {
                are_boxes_visible: true,
                ..Default::default()
            },
            coco_file: if let Some(cf) = cfg.coco_file {
                cf
            } else {
                CocoFile::default()
            },
        }
    }

    pub fn set_annotations_map(&mut self, map: AnnotationsMap) -> RvResult<()> {
        for (_, (annos, _)) in map.iter() {
            for cat_idx in annos.cat_idxs() {
                let len = self.labels().len();
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
    pub coco_file: CocoFile,
    pub is_export_absolute: bool,
}

impl BboxExportData {
    pub fn from_bbox_data(mut bbox_specifics: BboxSpecificData) -> Self {
        let coco_file = bbox_specifics.coco_file.clone();
        let is_export_absolute = bbox_specifics.options.export_absolute;
        BboxExportData {
            labels: mem::take(&mut bbox_specifics.labels),
            colors: mem::take(&mut bbox_specifics.colors),
            cat_ids: mem::take(&mut bbox_specifics.cat_ids),
            annotations: bbox_specifics
                .anno_intoiter()
                .map(|(filename, (annos, shape))| {
                    let (bbs, labels) = annos.to_data();
                    (filename, (bbs, labels, shape))
                })
                .collect::<HashMap<_, _>>(),
            coco_file,
            is_export_absolute,
        }
    }
}

#[test]
fn test_argmax() {
    let picklist = [
        [200, 200, 200u8],
        [1, 7, 3],
        [0, 0, 1],
        [45, 43, 52],
        [1, 10, 15],
    ];
    let legacylist = [
        [17, 16, 15],
        [199, 199, 201u8],
        [50, 50, 50u8],
        [255, 255, 255u8],
    ];
    assert_eq!(argmax_clr_dist(&picklist, &legacylist), [0, 0, 1]);
}
