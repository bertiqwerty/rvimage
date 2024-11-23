#[cfg(test)]
use super::annotations::InstanceAnnotations;
use super::{
    annotations::{BrushAnnotations, ClipboardData},
    core::{self, AnnotationsMap, CocoRle, CocoSegmentation, ExportAsCoco, LabelInfo},
    InstanceAnnotate, InstanceExportData,
};
use crate::{cfg::ExportPath, result::trace_ok_warn, BrushLine};
use crate::{implement_annotate, implement_annotations_getters};
use rvimage_domain::{
    access_mask_abs, access_mask_rel, mask_to_rle, rle_bb_to_image, rverr, BbF, Canvas, PtF, PtI,
    PtS, RvResult, ShapeI, TPtF, TPtI, TPtS, BB,
};

use serde::{Deserialize, Serialize};

pub type BrushAnnoMap = AnnotationsMap<Canvas>;

pub const MAX_THICKNESS: f64 = 100.0;
pub const MIN_THICKNESS: f64 = 1.0;
pub const MAX_INTENSITY: f64 = 1.0;
pub const MIN_INTENSITY: f64 = 0.01;
const fn default_alpha() -> u8 {
    255
}
const fn default_perfilecrowd() -> bool {
    false
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub thickness: TPtF,
    pub intensity: TPtF,
    #[serde(skip)]
    pub is_selection_change_needed: bool,
    #[serde(skip)]
    pub core_options: core::Options,
    #[serde(default = "default_alpha")]
    pub fill_alpha: u8,
    #[serde(default = "default_perfilecrowd")]
    pub per_file_crowd: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            thickness: 15.0,
            intensity: 0.5,
            is_selection_change_needed: false,
            core_options: core::Options::default(),
            fill_alpha: default_alpha(),
            per_file_crowd: default_perfilecrowd(),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, Default)]
pub struct BrushToolData {
    pub annotations_map: BrushAnnoMap,
    // we might want to show this while it is being drawn,
    // (line, cat_idx)
    #[serde(skip)]
    pub tmp_line: Option<(BrushLine, usize)>,
    pub options: Options,
    pub label_info: LabelInfo,
    #[serde(skip)]
    pub clipboard: Option<ClipboardData<Canvas>>,
    pub coco_file: ExportPath,
}
impl BrushToolData {
    implement_annotations_getters!(BrushAnnotations);
    pub fn from_coco_export_data(input_data: InstanceExportData<Canvas>) -> RvResult<Self> {
        let label_info = input_data.label_info()?;
        let mut out_data = Self {
            tmp_line: None,

            label_info,
            annotations_map: AnnotationsMap::new(),
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

impl<'de> Deserialize<'de> for BrushToolData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct BrushToolDataDe {
            annotations_map: BrushAnnoMap,
            options: Options,
            label_info: LabelInfo,
            coco_file: Option<ExportPath>,
        }

        let read = BrushToolDataDe::deserialize(deserializer)?;
        Ok(Self {
            annotations_map: read.annotations_map,
            tmp_line: None,
            options: read.options,
            label_info: read.label_info,
            clipboard: None,
            coco_file: read.coco_file.unwrap_or_default(),
        })
    }
}

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
    fn label_info(&self) -> &LabelInfo {
        &self.label_info
    }
    fn core_options_mut(&mut self) -> &mut core::Options {
        &mut self.options.core_options
    }
    fn new(
        options: core::Options,
        label_info: LabelInfo,
        anno_map: AnnotationsMap<Canvas>,
        export_path: ExportPath,
    ) -> Self {
        Self {
            annotations_map: anno_map,
            tmp_line: None,
            options: Options {
                core_options: options,
                ..Default::default()
            },
            label_info,
            clipboard: None,
            coco_file: export_path,
        }
    }
    fn set_annotations_map(&mut self, map: AnnotationsMap<Canvas>) -> RvResult<()> {
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
    fn set_labelinfo(&mut self, info: LabelInfo) {
        self.label_info = info;
    }
    #[cfg(test)]
    fn anno_iter(&self) -> impl Iterator<Item = (&String, &(InstanceAnnotations<Canvas>, ShapeI))> {
        self.anno_iter()
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
    fn rot90_with_image_ntimes(self, shape: &ShapeI, n: u8) -> RvResult<Self> {
        let bb = self.bb;
        let bb_s: BB<TPtS> = BB::from(self.bb);
        let bb_rot = bb_s.rot90_with_image_ntimes(shape, n);
        if bb_rot.x < 0 || bb_rot.y < 0 {
            return Err(rverr!("rotated bb {bb_rot:?} has negative coordinates",));
        }
        let mut new_mask = self.mask.clone();
        for y in 0..bb.h {
            for x in 0..bb.w {
                let p_mask = PtI { x, y };
                let p_im = p_mask + bb.min();
                let p_im_rot = PtS::from(p_im).rot90_with_image_ntimes(shape, n);
                let p_newmask = p_im_rot - bb_rot.min();
                let p_newmask: PtI = p_newmask.into();
                new_mask[p_newmask.y as usize * bb_rot.w as usize + p_newmask.x as usize] =
                    self.mask[p_mask.y as usize * bb.w as usize + p_mask.x as usize];
            }
        }
        Ok(Self {
            mask: new_mask,
            bb: bb_rot.into(),
            intensity: self.intensity,
        })
    }
    fn to_cocoseg(
        &self,
        shape_im: ShapeI,
        _is_export_absolute: bool,
    ) -> RvResult<Option<core::CocoSegmentation>> {
        if !self.bb.is_contained_in_image(shape_im) {
            Err(rverr!(
                "bb {:?} not contained in image {shape_im:?}",
                self.bb
            ))
        } else {
            let rle_bb = mask_to_rle(&self.mask, self.bb.w, self.bb.h);

            let rle_im = trace_ok_warn(rle_bb_to_image(&rle_bb, self.bb, shape_im));
            Ok(rle_im.map(|rle_im| {
                CocoSegmentation::Rle(CocoRle {
                    counts: rle_im,
                    size: (shape_im.w, shape_im.h),
                    intensity: Some(self.intensity),
                })
            }))
        }
    }
    /// Returns the distance to the boundary of the mask
    ///
    /// *Arguments*:
    /// p: in image coordinates
    fn dist_to_boundary(&self, p: PtF) -> TPtF {
        let mut min_dist = TPtF::MAX;
        let to_coord = |x| {
            if x > 0.0 {
                x as TPtI
            } else {
                TPtI::MAX
            }
        };
        // we need this to check whether p is a foreground pixel in case
        // it inside the bounding box of the canvas
        let point_pixel_inside = PtI {
            x: to_coord(p.x - self.bb.x as TPtF),
            y: to_coord(p.y - self.bb.y as TPtF),
        };
        let point_pixel_value = access_mask_rel(
            &self.mask,
            point_pixel_inside.x,
            point_pixel_inside.y,
            self.bb.w,
            self.bb.h,
        );
        for y in 1..self.bb.h {
            for x in 1..self.bb.w {
                let neighbors_fg_mask = [
                    access_mask_rel(&self.mask, x + 1, y, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x - 1, y, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x, y + 1, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x, y - 1, self.bb.w, self.bb.h),
                ];
                if neighbors_fg_mask.iter().any(|&b| b != point_pixel_value) {
                    let x = (x + self.bb.x) as TPtF;
                    let y = (y + self.bb.y) as TPtF;
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

implement_annotate!(BrushToolData);

#[cfg(test)]
use rvimage_domain::{BbI, Line};
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
    let canv = Canvas::new(&bl, orig_shape).unwrap();
    assert!(canv.contains(PtF { x: 5.0, y: 5.0 }));
    assert!(!canv.contains(PtF { x: 0.0, y: 0.0 }));
    assert!(canv.contains(PtF { x: 14.9, y: 14.9 }));
    assert!(!canv.contains(PtF { x: 0.0, y: 9.9 }));
    assert!(!canv.contains(PtF { x: 15.0, y: 15.0 }));
    let d = canv.dist_to_boundary(PtF { x: 5.0, y: 5.0 });
    assert!((d - 1.0).abs() < 1e-8);
    let dist = canv.dist_to_boundary(PtF { x: 5.0, y: 15.0 });
    assert!(5.0 < dist && dist < 7.0);
    for y in canv.bb.y_range() {
        for x in canv.bb.x_range() {
            _ = access_mask_abs(&canv.mask, canv.bb, PtI { x, y });
        }
    }
    let canv = Canvas::new(&bl, orig_shape).unwrap();
    let canv_rot = canv
        .clone()
        .rot90_with_image_ntimes(&orig_shape, 1)
        .unwrap();
    let bl_rot = BrushLine {
        line: Line {
            points: vec![
                PtF { x: 5.0, y: 5.0 }.rot90_with_image_ntimes(&orig_shape, 1),
                PtF { x: 15.0, y: 15.0 }.rot90_with_image_ntimes(&orig_shape, 1),
            ],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    let canv_rot_ref = Canvas::new(&bl_rot, orig_shape).unwrap();
    let inter = canv_rot
        .enclosing_bb()
        .intersect(canv_rot_ref.enclosing_bb());
    assert!(
        (inter.w - canv_rot.enclosing_bb().w).abs() <= 1.0
            && (inter.h - canv_rot.enclosing_bb().h).abs() <= 1.0
    );
    let canv = Canvas::new(&bl, orig_shape).unwrap();
    assert_eq!(
        canv,
        canv.clone()
            .rot90_with_image_ntimes(&orig_shape, 0)
            .unwrap()
    );
}

#[test]
fn test_canvas_rot() {
    let canv = Canvas {
        mask: vec![0, 0, 0, 1],
        bb: BbI::from_arr(&[0, 0, 4, 1]),
        intensity: 0.5,
    };
    let canv_rot = canv
        .clone()
        .rot90_with_image_ntimes(&ShapeI::new(4, 1), 1)
        .unwrap();
    let canv_ref = Canvas {
        mask: vec![1, 0, 0, 0],
        bb: BbI::from_arr(&[0, 0, 1, 4]),
        intensity: 0.5,
    };
    assert_eq!(canv_rot, canv_ref);
}
