use std::iter;

use serde::{Deserialize, Serialize};

#[cfg(test)]
use super::annotations::InstanceAnnotations;
use super::{
    annotations::{BboxAnnotations, ClipboardData},
    core::{
        AnnotationsMap, CocoSegmentation, ExportAsCoco, InstanceExportData, LabelInfo,
        OUTLINE_THICKNESS_CONVERSION,
    },
    InstanceAnnotate,
};
use crate::{
    cfg::ExportPath,
    file_util, implement_annotations_getters,
    tools_data::{annotations::SplitMode, core},
    GeoFig,
};
use rvimage_domain::{rverr, BbF, Circle, PtF, RvResult, ShapeI, TPtF};

/// filename -> (annotations per file, file dimensions)
pub type BboxAnnoMap = AnnotationsMap<GeoFig>;

#[derive(Clone, Copy, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct Options {
    #[serde(skip)]
    pub core_options: core::Options,
    #[serde(skip)]
    pub is_anno_outoffolder_rm_triggered: bool,
    pub split_mode: SplitMode,
    pub fill_alpha: u8,
    pub outline_alpha: u8,
    pub outline_thickness: u16,
    pub drawing_distance: u8,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            core_options: core::Options::default(),
            is_anno_outoffolder_rm_triggered: false,
            split_mode: SplitMode::default(),
            fill_alpha: 30,
            outline_alpha: 255,
            outline_thickness: OUTLINE_THICKNESS_CONVERSION as u16,
            drawing_distance: 10,
        }
    }
}
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct BboxSpecificData {
    pub label_info: LabelInfo,
    pub annotations_map: BboxAnnoMap,
    #[serde(skip)]
    pub clipboard: Option<ClipboardData<GeoFig>>,
    pub options: Options,
    pub coco_file: ExportPath,
    #[serde(skip)]
    pub highlight_circles: Vec<Circle>,
}

impl BboxSpecificData {
    implement_annotations_getters!(BboxAnnotations);

    pub fn separate_data(self) -> (LabelInfo, BboxAnnoMap, ExportPath) {
        (self.label_info, self.annotations_map, self.coco_file)
    }

    pub fn from_coco_export_data(input_data: InstanceExportData<GeoFig>) -> RvResult<Self> {
        let label_info = input_data.label_info()?;
        let mut out_data = Self {
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
            highlight_circles: vec![],
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

        BboxSpecificData {
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
            coco_file: ExportPath::default(),
            highlight_circles: vec![],
        }
    }

    pub fn set_annotations_map(&mut self, map: BboxAnnoMap) -> RvResult<()> {
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

impl ExportAsCoco<GeoFig> for BboxSpecificData {
    fn cocofile_conn(&self) -> ExportPath {
        self.coco_file.clone()
    }
    fn separate_data(self) -> (core::Options, LabelInfo, AnnotationsMap<GeoFig>, ExportPath) {
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
    #[cfg(test)]
    fn anno_iter(&self) -> impl Iterator<Item = (&String, &(InstanceAnnotations<GeoFig>, ShapeI))> {
        self.anno_iter()
    }
    fn core_options_mut(&mut self) -> &mut core::Options {
        &mut self.options.core_options
    }
    fn new(
        options: core::Options,
        label_info: LabelInfo,
        anno_map: AnnotationsMap<GeoFig>,
        export_path: ExportPath,
    ) -> Self {
        Self {
            label_info,
            annotations_map: anno_map,
            clipboard: None,
            options: Options {
                core_options: options,
                ..Default::default()
            },
            coco_file: export_path,
            highlight_circles: vec![],
        }
    }
    fn set_annotations_map(&mut self, map: AnnotationsMap<GeoFig>) -> RvResult<()> {
        self.set_annotations_map(map)
    }
    fn set_labelinfo(&mut self, info: LabelInfo) {
        self.label_info = info;
    }
}

impl InstanceAnnotate for GeoFig {
    fn rot90_with_image_ntimes(self, shape: &ShapeI, n: u8) -> RvResult<Self> {
        Ok(match self {
            Self::BB(bb) => Self::BB(bb.rot90_with_image_ntimes(shape, n)),
            Self::Poly(poly) => Self::Poly(poly.rot90_with_image_ntimes(shape, n)),
        })
    }
    fn is_contained_in_image(&self, shape: ShapeI) -> bool {
        match self {
            Self::BB(bb) => bb.is_contained_in_image(shape),
            Self::Poly(poly) => poly.is_contained_in_image(shape),
        }
    }
    fn enclosing_bb(&self) -> BbF {
        match self {
            Self::BB(bb) => *bb,
            Self::Poly(poly) => poly.enclosing_bb(),
        }
    }
    fn contains<P>(&self, point: P) -> bool
    where
        P: Into<PtF>,
    {
        match self {
            Self::BB(bb) => bb.contains(point.into()),
            Self::Poly(poly) => poly.contains(point),
        }
    }
    fn dist_to_boundary(&self, point: PtF) -> TPtF {
        match self {
            Self::BB(bb) => bb.distance_to_boundary(point),
            Self::Poly(poly) => poly.distance_to_boundary(point),
        }
    }
    fn to_cocoseg(
        &self,
        shape_im: ShapeI,
        is_export_absolute: bool,
    ) -> RvResult<Option<core::CocoSegmentation>> {
        Ok(Some(CocoSegmentation::Polygon(vec![
            if is_export_absolute {
                self.points()
            } else {
                self.points_normalized(shape_im.w as TPtF, shape_im.h as TPtF)
            }
            .iter()
            .flat_map(|p| iter::once(p.x).chain(iter::once(p.y)))
            .collect::<Vec<_>>(),
        ])))
    }
}
