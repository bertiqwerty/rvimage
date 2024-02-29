use std::collections::HashMap;

use super::{
    annotations::{BrushAnnotations, ClipboardData},
    core::{self, AnnotationsMap, CocoRle, CocoSegmentation, ExportAsCoco, LabelInfo},
    InstanceAnnotate, InstanceExportData,
};
use crate::{
    cfg::ExportPath,
    domain::{
        access_mask_abs, access_mask_rel, mask_to_rle, rle_bb_to_image, BbF, Canvas, Point, PtF,
        PtI, ShapeI, TPtI, BB,
    },
    result::{trace_ok, RvResult},
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
    pub coco_file: ExportPath,
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
    pub fn from_coco_export_data(input_data: InstanceExportData<Canvas>) -> RvResult<Self> {
        let label_info = input_data.label_info()?;
        let mut out_data = Self {
            tmp_line: None,

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
                .map(|(s, (canvases, cat_ids, dims))| {
                    (
                        s,
                        (BrushAnnotations::from_elts_cats(canvases, cat_ids), dims),
                    )
                })
                .collect(),
        )?;
        Ok(out_data)
    }
}
impl Eq for BrushToolData {}

impl ExportAsCoco<Canvas> for BrushToolData {
    fn cocofile_conn(&self) -> ExportPath {
        self.coco_file.clone()
    }
    fn separate_data(self) -> (core::Options, LabelInfo, AnnotationsMap<Canvas>, ExportPath) {
        (
            self.options.core_options,
            self.label_info,
            self.annotations_map,
            self.coco_file,
        )
    }
}

impl InstanceAnnotate for Canvas {
    fn is_contained_in_image(&self, shape: crate::ShapeI) -> bool {
        self.bb.is_contained_in_image(shape)
    }
    fn contains<P>(&self, point: P) -> bool
    where
        P: Into<PtF>,
    {
        let p_tmp: PtF = point.into();
        let p_idx: PtI = p_tmp.into();
        access_mask_abs(&self.mask, self.bb, p_idx) > 0
    }
    fn enclosing_bb(&self) -> BbF {
        self.bb.into()
    }
    fn rot90_with_image_ntimes(self, shape: &ShapeI, n: u8) -> Self {
        let bb = self.bb;
        let bb_f: BbF = BB::from(self.bb);
        let bb_rot = bb_f.rot90_with_image_ntimes(shape, n);
        let mut new_mask = self.mask.clone();
        for y in 0..bb.h {
            for x in 0..bb.w {
                let p = Point { x, y } + bb.min();
                let p_rot = PtF::from(p).rot90_with_image_ntimes(shape, n);
                let p_newmask = p_rot - bb_rot.min();
                let p_newmask: PtI = p_newmask.into();
                new_mask[p_newmask.y as usize * bb_rot.w as usize + p_newmask.x as usize] =
                    self.mask[p.y as usize * bb.w as usize + p.x as usize];
            }
        }
        Self {
            mask: new_mask,
            bb: bb_rot.into(),
            intensity: self.intensity,
        }
    }
    fn to_cocoseg(&self, w_im: TPtI, h_im: TPtI) -> Option<core::CocoSegmentation> {
        let rle_bb = mask_to_rle(&self.mask, self.bb.w, self.bb.h);
        let rle_im = trace_ok(rle_bb_to_image(&rle_bb, self.bb, ShapeI::new(w_im, h_im)));
        rle_im.map(|rle_im| {
            CocoSegmentation::Rle(CocoRle {
                counts: rle_im,
                size: (w_im, h_im),
                intensity: Some(self.intensity),
            })
        })
    }
    fn dist_to_boundary(&self, p: PtF) -> TPtF {
        let mut min_dist = TPtF::MAX;
        for y in 0..self.bb.h {
            for x in 0..self.bb.w {
                let is_current_foreground = access_mask_rel(&self.mask, x, y, self.bb.w, self.bb.h);
                let neighbors_fg_mask = [
                    access_mask_rel(&self.mask, x + 1, y, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x.wrapping_sub(1), y, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x, y + 1, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x, y.wrapping_sub(1), self.bb.w, self.bb.h),
                ];
                if neighbors_fg_mask
                    .iter()
                    .any(|&b| b != is_current_foreground)
                {
                    let x = x as TPtF;
                    let y = y as TPtF;
                    let dist = p.dist_square(&PtF { x, y }).sqrt();
                    if dist < min_dist {
                        min_dist = dist;
                    }
                }
            }
        }
        min_dist
    }
}
#[cfg(test)]
use crate::domain::Line;
#[test]
fn test_canvas() {
    let orig_shape = ShapeI::new(30, 30);
    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 5.0, y: 5.0 }, PtF { x: 15.0, y: 15.0 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    let cv = Canvas::new(&bl, orig_shape).unwrap();
    assert!(cv.contains(PtF { x: 5.0, y: 5.0 }));
    assert!(!cv.contains(PtF { x: 0.0, y: 0.0 }));
    assert!(cv.contains(PtF { x: 14.9, y: 14.9 }));
    assert!(!cv.contains(PtF { x: 0.0, y: 9.9 }));
    assert!(!cv.contains(PtF { x: 15.0, y: 15.0 }));

    assert!((cv.dist_to_boundary(PtF { x: 5.0, y: 5.0 }) - 1.0).abs() < 1e-8);
    let dist = cv.dist_to_boundary(PtF { x: 5.0, y: 15.0 });
    assert!(5.0 < dist && dist < 7.0);
    for y in cv.bb.y_range() {
        for x in cv.bb.x_range() {
            access_mask_abs(&cv.mask, cv.bb, PtI { x, y });
        }
    }
}
