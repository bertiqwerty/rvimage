use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use tracing::info;

use crate::{cfg::ExportPath, util::Visibility, ShapeI};
use rvimage_domain::{
    rle_image_to_bb, rle_to_mask, rverr, Canvas, GeoFig, Point, Polygon, RvResult,
};
use rvimage_domain::{BbF, PtF, TPtF, TPtI};

use super::annotations::InstanceAnnotations;
use super::label_map::LabelMap;

pub const OUTLINE_THICKNESS_CONVERSION: TPtF = 10.0;

const DEFAULT_LABEL: &str = "rvimage_fg";

fn color_dist(c1: [u8; 3], c2: [u8; 3]) -> f32 {
    let square_d = |i| (f32::from(c1[i]) - f32::from(c2[i])).powi(2);
    (square_d(0) + square_d(1) + square_d(2)).sqrt()
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportMode {
    Merge,
    #[default]
    Replace,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportExportTrigger {
    export_triggered: bool,
    import_triggered: bool,
    import_mode: ImportMode,
}
impl ImportExportTrigger {
    pub fn import_triggered(self) -> bool {
        self.import_triggered
    }
    pub fn import_mode(self) -> ImportMode {
        self.import_mode
    }
    pub fn export_triggered(self) -> bool {
        self.export_triggered
    }
    pub fn untrigger_export(&mut self) {
        self.export_triggered = false;
    }
    pub fn untrigger_import(&mut self) {
        self.import_triggered = false;
    }
    pub fn trigger_export(&mut self) {
        self.export_triggered = true;
    }
    pub fn trigger_import(&mut self) {
        self.import_triggered = true;
    }
    pub fn use_merge_import(&mut self) {
        self.import_mode = ImportMode::Merge;
    }
    pub fn use_replace_import(&mut self) {
        self.import_mode = ImportMode::Replace;
    }
    pub fn merge_mode(self) -> bool {
        self.import_mode == ImportMode::Merge
    }
    pub fn from_export_triggered(export_triggered: bool) -> Self {
        Self {
            export_triggered,
            ..Default::default()
        }
    }
}

pub type AnnotationsMap<T> = LabelMap<InstanceAnnotations<T>>;

fn sort<T>(annos: InstanceAnnotations<T>, access_x_or_y: fn(BbF) -> TPtF) -> InstanceAnnotations<T>
where
    T: InstanceAnnotate,
{
    let (elts, cat_idxs, selected_mask) = annos.separate_data();
    let mut tmp_tuples = elts
        .into_iter()
        .zip(cat_idxs)
        .zip(selected_mask)
        .collect::<Vec<_>>();
    tmp_tuples.sort_by(|((elt1, _), _), ((elt2, _), _)| {
        match access_x_or_y(elt1.enclosing_bb()).partial_cmp(&access_x_or_y(elt2.enclosing_bb())) {
            Some(o) => o,
            None => {
                tracing::error!(
                    "there is a NAN in an annotation box {:?}, {:?}",
                    elt1.enclosing_bb(),
                    elt2.enclosing_bb()
                );
                std::cmp::Ordering::Equal
            }
        }
    });
    InstanceAnnotations::from_tuples(tmp_tuples)
}

/// Small little labels to be displayed in a box below instance annotations
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InstanceLabelDisplay {
    #[default]
    None,
    // count from left to right
    IndexLr,
    // count from top to bottom
    IndexTb,
    // category label
    CatLabel,
}

impl InstanceLabelDisplay {
    pub fn next(self) -> Self {
        match self {
            Self::None => Self::IndexLr,
            Self::IndexLr => Self::IndexTb,
            Self::IndexTb => Self::CatLabel,
            Self::CatLabel => Self::None,
        }
    }
    pub fn sort<T>(self, annos: InstanceAnnotations<T>) -> InstanceAnnotations<T>
    where
        T: InstanceAnnotate,
    {
        match self {
            Self::None | Self::CatLabel => annos,
            Self::IndexLr => sort(annos, |bb| bb.x),
            Self::IndexTb => sort(annos, |bb| bb.y),
        }
    }
}
impl Display for InstanceLabelDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::IndexLr => write!(f, "Index-Left-Right"),
            Self::IndexTb => write!(f, "Index-Top-Bottom"),
            Self::CatLabel => write!(f, "Category-Label"),
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub visible: bool,
    pub is_colorchange_triggered: bool,
    pub is_redraw_annos_triggered: bool,
    pub is_export_absolute: bool,
    pub import_export_trigger: ImportExportTrigger,
    pub is_history_update_triggered: bool,
    pub track_changes: bool,
    pub erase: bool,
    pub label_propagation: Option<usize>,
    pub label_deletion: Option<usize>,
    pub auto_paste: bool,
    pub instance_label_display: InstanceLabelDisplay,
    pub doublecheck_cocoexport_shape: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            visible: true,
            is_colorchange_triggered: false,
            is_redraw_annos_triggered: false,
            is_export_absolute: false,
            import_export_trigger: ImportExportTrigger::default(),
            is_history_update_triggered: false,
            track_changes: false,
            erase: false,
            label_propagation: None,
            label_deletion: None,
            auto_paste: false,
            instance_label_display: InstanceLabelDisplay::None,
            doublecheck_cocoexport_shape: true,
        }
    }
}
impl Options {
    pub fn trigger_redraw_and_hist(mut self) -> Self {
        self.is_history_update_triggered = true;
        self.is_redraw_annos_triggered = true;
        self
    }
}

const N: usize = 1;
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct VisibleInactiveToolsState {
    // should the tool's annotations be shown in the background
    show_mask: [bool; N],
}
impl VisibleInactiveToolsState {
    pub fn new() -> Self {
        Self::default()
    }
    #[allow(clippy::needless_lifetimes)]
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = bool> + 'a {
        self.show_mask.iter().copied()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut bool> {
        self.show_mask.iter_mut()
    }
    pub fn hide_all(&mut self) {
        for show in &mut self.show_mask {
            *show = false;
        }
    }
    pub fn set_show(&mut self, idx: usize, is_visible: bool) {
        self.show_mask[idx] = is_visible;
    }
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

fn get_visibility(visible: bool, show_only_current: bool, cat_idx_current: usize) -> Visibility {
    if visible && show_only_current {
        Visibility::Only(cat_idx_current)
    } else if visible {
        Visibility::All
    } else {
        Visibility::None
    }
}

pub fn vis_from_lfoption(label_info: Option<&LabelInfo>, visible: bool) -> Visibility {
    if let Some(label_info) = label_info {
        label_info.visibility(visible)
    } else if visible {
        Visibility::All
    } else {
        Visibility::None
    }
}

pub fn merge<T>(
    annos1: AnnotationsMap<T>,
    li1: LabelInfo,
    annos2: AnnotationsMap<T>,
    li2: LabelInfo,
) -> (AnnotationsMap<T>, LabelInfo)
where
    T: InstanceAnnotate,
{
    let (li, idx_map) = li1.merge(li2);
    let mut annotations_map = annos1;

    for (k, (v2, s)) in annos2 {
        if let Some((v1, _)) = annotations_map.get_mut(&k) {
            let (elts, cat_idxs, _) = v2.separate_data();
            v1.extend(
                elts.into_iter(),
                cat_idxs.into_iter().map(|old_idx| idx_map[old_idx]),
                s,
                InstanceLabelDisplay::default(),
            );
            v1.deselect_all();
        } else {
            let (elts, cat_idxs, _) = v2.separate_data();
            let cat_idxs = cat_idxs
                .into_iter()
                .map(|old_idx| idx_map[old_idx])
                .collect::<Vec<_>>();
            let v2 =
                InstanceAnnotations::new_relaxed(elts, cat_idxs, InstanceLabelDisplay::default());
            annotations_map.insert(k, (v2, s));
        }
    }
    (annotations_map, li)
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct LabelInfo {
    pub new_label: String,
    labels: Vec<String>,
    colors: Vec<[u8; 3]>,
    cat_ids: Vec<u32>,
    pub cat_idx_current: usize,
    pub show_only_current: bool,
}
impl LabelInfo {
    /// Merges two `LabelInfo`s. Returns the merged `LabelInfo` and a vector that maps
    /// the indices of the second `LabelInfo` to the indices of the merged `LabelInfo`.
    pub fn merge(mut self, other: Self) -> (Self, Vec<usize>) {
        let mut idx_map = vec![];
        for other_label in other.labels {
            let self_cat_idx = self.labels.iter().position(|slab| slab == &other_label);
            if let Some(scidx) = self_cat_idx {
                idx_map.push(scidx);
            } else {
                self.labels.push(other_label);
                self.colors.push(new_color(&self.colors));
                self.cat_ids.push(self.labels.len() as u32);
                idx_map.push(self.labels.len() - 1);
            }
        }
        (self, idx_map)
    }

    pub fn visibility(&self, visible: bool) -> Visibility {
        get_visibility(visible, self.show_only_current, self.cat_idx_current)
    }
    pub fn new_random_colors(&mut self) {
        info!("new random colors for annotations");
        self.colors = new_random_colors(self.colors.len());
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
            info!("adding label '{label}'");
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
    pub fn rename_label(&mut self, idx: usize, label: String) -> RvResult<()> {
        if self.labels.contains(&label) {
            Err(rverr!("label '{label}' already exists"))
        } else {
            self.labels[idx] = label;
            Ok(())
        }
    }
    pub fn from_iter(it: impl Iterator<Item = ((String, [u8; 3]), u32)>) -> RvResult<Self> {
        let mut info = Self::empty();
        for ((label, color), cat_id) in it {
            info.push(label, Some(color), Some(cat_id))?;
        }
        Ok(info)
    }
    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }
    pub fn len(&self) -> usize {
        self.labels.len()
    }
    pub fn remove(&mut self, idx: usize) -> (String, [u8; 3], u32) {
        let removed_items = (
            self.labels.remove(idx),
            self.colors.remove(idx),
            self.cat_ids.remove(idx),
        );
        info!("label '{}' removed", removed_items.0);
        removed_items
    }
    pub fn find_default(&mut self) -> Option<&mut String> {
        self.labels.iter_mut().find(|lab| lab == &DEFAULT_LABEL)
    }
    pub fn colors(&self) -> &Vec<[u8; 3]> {
        &self.colors
    }

    pub fn labels(&self) -> &Vec<String> {
        &self.labels
    }

    pub fn cat_ids(&self) -> &Vec<u32> {
        &self.cat_ids
    }

    pub fn separate_data(self) -> (Vec<String>, Vec<[u8; 3]>, Vec<u32>) {
        (self.labels, self.colors, self.cat_ids)
    }

    pub fn empty() -> Self {
        Self {
            new_label: DEFAULT_LABEL.to_string(),
            labels: vec![],
            colors: vec![],
            cat_ids: vec![],
            cat_idx_current: 0,
            show_only_current: false,
        }
    }
    pub fn remove_catidx<'a, T>(&mut self, cat_idx: usize, annotaions_map: &mut AnnotationsMap<T>)
    where
        T: InstanceAnnotate + PartialEq + Default + 'a,
    {
        if self.len() > 1 {
            self.remove(cat_idx);
            if self.cat_idx_current >= cat_idx.max(1) {
                self.cat_idx_current -= 1;
            }
            for (anno, _) in annotaions_map.values_mut() {
                let indices_for_rm = anno
                    .cat_idxs()
                    .iter()
                    .enumerate()
                    .filter(|(_, geo_cat_idx)| **geo_cat_idx == cat_idx)
                    .map(|(idx, _)| idx)
                    .collect::<Vec<_>>();
                anno.remove_multiple(&indices_for_rm);
                anno.reduce_cat_idxs(cat_idx);
            }
        }
    }
}

impl Default for LabelInfo {
    fn default() -> Self {
        let new_label = DEFAULT_LABEL.to_string();
        let new_color = [255, 255, 255];
        let labels = vec![new_label.clone()];
        let colors = vec![new_color];
        let cat_ids = vec![1];
        Self {
            new_label,
            labels,
            colors,
            cat_ids,
            cat_idx_current: 0,
            show_only_current: false,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct InstanceExportData<A> {
    pub labels: Vec<String>,
    pub colors: Vec<[u8; 3]>,
    pub cat_ids: Vec<u32>,
    // filename, bounding boxes, classes of the boxes, dimensions of the image
    pub annotations: HashMap<String, (Vec<A>, Vec<usize>, ShapeI)>,
    pub coco_file: ExportPath,
    pub is_export_absolute: bool,
}

impl<A> InstanceExportData<A>
where
    A: InstanceAnnotate,
{
    pub fn from_tools_data(
        options: &Options,
        label_info: LabelInfo,
        coco_file: ExportPath,
        annotations_map: AnnotationsMap<A>,
    ) -> Self {
        let is_export_absolute = options.is_export_absolute;
        let annotations = annotations_map
            .into_iter()
            .map(|(filename, (annos, shape))| {
                let (bbs, labels, _) = annos.separate_data();
                (filename, (bbs, labels, shape))
            })
            .collect::<HashMap<_, _>>();
        let (labels, colors, cat_ids) = label_info.separate_data();
        InstanceExportData {
            labels,
            colors,
            cat_ids,
            annotations,
            coco_file,
            is_export_absolute,
        }
    }
    pub fn label_info(&self) -> RvResult<LabelInfo> {
        LabelInfo::from_iter(
            self.labels
                .clone()
                .into_iter()
                .zip(self.colors.clone())
                .zip(self.cat_ids.clone()),
        )
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct CocoRle {
    pub counts: Vec<TPtI>,
    pub size: (TPtI, TPtI),
    pub intensity: Option<TPtF>,
}

impl CocoRle {
    pub fn to_canvas(&self, bb: BbF) -> RvResult<Canvas> {
        let bb = bb.into();
        let rle_bb = rle_image_to_bb(&self.counts, bb, ShapeI::from(self.size))?;
        let mask = rle_to_mask(&rle_bb, bb.w, bb.h);
        let intensity = self.intensity.unwrap_or(1.0);
        Ok(Canvas {
            bb,
            mask,
            intensity,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CocoSegmentation {
    Polygon(Vec<Vec<TPtF>>),
    Rle(CocoRle),
}

pub fn polygon_to_geofig(
    poly: &[Vec<TPtF>],
    w_factor: f64,
    h_factor: f64,
    bb: BbF,
    mut warn: impl FnMut(&str),
) -> RvResult<GeoFig> {
    if poly.len() > 1 {
        return Err(rverr!(
            "multiple polygons per box not supported. ignoring all but first."
        ));
    }
    let n_points = poly[0].len();
    let coco_data = &poly[0];

    let poly_points = (0..n_points)
        .step_by(2)
        .filter_map(|idx| {
            let p = Point {
                x: (coco_data[idx] * w_factor),
                y: (coco_data[idx + 1] * h_factor),
            };
            if bb.contains(p) {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    let poly = Polygon::from_vec(poly_points);
    if let Ok(poly) = poly {
        let encl_bb = poly.enclosing_bb();
        if encl_bb.w * encl_bb.h < 1e-6 && bb.w * bb.h > 1e-6 {
            warn(&format!(
                "polygon has no area. using bb. bb: {bb:?}, poly: {encl_bb:?}"
            ));
            Ok(GeoFig::BB(bb))
        } else {
            if !bb.all_corners_close(encl_bb) {
                let msg = format!(
                    "bounding box and polygon enclosing box do not match. using bb. bb: {bb:?}, poly: {encl_bb:?}"
                );
                warn(&msg);
            }
            // check if the poly is just a bounding box
            if poly.points().len() == 4
                                // all points are bb corners
                                && poly.points_iter().all(|p| {
                                    encl_bb.points_iter().any(|p_encl| p == p_encl)})
                                // all points are different
                                && poly
                                    .points_iter()
                                    .all(|p| poly.points_iter().filter(|p_| p == *p_).count() == 1)
            {
                Ok(GeoFig::BB(bb))
            } else {
                Ok(GeoFig::Poly(poly))
            }
        }
    } else if n_points > 0 {
        Err(rverr!(
            "Segmentation invalid, could not be created from polygon with {n_points} points"
        ))
    } else {
        // polygon might be empty, we continue with the BB
        Ok(GeoFig::BB(bb))
    }
}

#[macro_export]
macro_rules! implement_annotate {
    ($tooldata:ident) => {
        impl $crate::tools_data::core::Annotate for $tooldata {
            fn has_annos(&self, relative_path: &str) -> bool {
                if let Some(v) = self.get_annos(relative_path) {
                    !v.is_empty()
                } else {
                    false
                }
            }
        }
    };
}

pub trait Annotate {
    /// Has the image with the given path annotations of the
    /// trait-implementing tool?
    fn has_annos(&self, relative_path: &str) -> bool;
}

pub trait InstanceAnnotate:
    Clone + Default + Debug + PartialEq + Serialize + DeserializeOwned
{
    fn is_contained_in_image(&self, shape: ShapeI) -> bool;
    fn contains<P>(&self, point: P) -> bool
    where
        P: Into<PtF>;
    fn dist_to_boundary(&self, p: PtF) -> TPtF;
    /// # Errors
    /// Can fail if a bounding box ends up with negative coordinates after rotation
    fn rot90_with_image_ntimes(self, shape: ShapeI, n: u8) -> RvResult<Self>;
    fn enclosing_bb(&self) -> BbF;
    /// # Errors
    /// Can fail if a bounding box is not on the image.
    fn to_cocoseg(
        &self,
        shape_im: ShapeI,
        is_export_absolute: bool,
    ) -> RvResult<Option<CocoSegmentation>>;
}
pub trait AccessInstanceData<T: InstanceAnnotate> {
    fn annotations_map(&self) -> &AnnotationsMap<T>;
    fn label_info(&self) -> &LabelInfo;
}
pub trait ExportAsCoco<A>: AccessInstanceData<A>
where
    A: InstanceAnnotate + 'static,
{
    fn cocofile_conn(&self) -> ExportPath;
    fn separate_data(self) -> (Options, LabelInfo, AnnotationsMap<A>, ExportPath);
    #[cfg(test)]
    fn anno_iter(&self) -> impl Iterator<Item = (&String, &(InstanceAnnotations<A>, ShapeI))>;
    fn set_annotations_map(&mut self, map: AnnotationsMap<A>) -> RvResult<()>;
    fn set_labelinfo(&mut self, info: LabelInfo);
    fn core_options_mut(&mut self) -> &mut Options;
    fn new(
        options: Options,
        label_info: LabelInfo,
        anno_map: AnnotationsMap<A>,
        export_path: ExportPath,
    ) -> Self;
}

#[cfg(test)]
use crate::tools_data::brush_data;
#[cfg(test)]
use rvimage_domain::{BrushLine, Line};
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

#[test]
fn test_labelinfo_merge() {
    let li1 = LabelInfo::default();
    let mut li2 = LabelInfo::default();
    li2.new_random_colors();
    let (mut li_merged, _) = li1.clone().merge(li2);
    assert_eq!(li1, li_merged);
    li_merged
        .push("new_label".into(), Some([0, 0, 1]), None)
        .unwrap();
    let (li_merged, _) = li_merged.merge(li1);
    let li_reference = LabelInfo {
        new_label: DEFAULT_LABEL.to_string(),
        labels: vec![DEFAULT_LABEL.to_string(), "new_label".to_string()],
        colors: vec![[255, 255, 255], [0, 0, 1]],
        cat_ids: vec![1, 2],
        cat_idx_current: 0,
        show_only_current: false,
    };
    assert_eq!(li_merged, li_reference);
    assert_eq!(li_merged.clone().merge(li_merged.clone()).0, li_reference);
    let li = LabelInfo {
        new_label: DEFAULT_LABEL.to_string(),
        labels: vec!["somelabel".to_string(), "new_label".to_string()],
        colors: vec![[255, 255, 255], [0, 1, 1]],
        cat_ids: vec![1, 2],
        cat_idx_current: 0,
        show_only_current: false,
    };
    let li_merged_ = li_merged.clone().merge(li.clone());
    let li_reference = (
        LabelInfo {
            new_label: DEFAULT_LABEL.to_string(),
            labels: vec![
                DEFAULT_LABEL.to_string(),
                "new_label".to_string(),
                "somelabel".to_string(),
            ],
            colors: vec![[255, 255, 255], [0, 0, 1], li_merged_.0.colors[2]],
            cat_ids: vec![1, 2, 3],
            cat_idx_current: 0,
            show_only_current: false,
        },
        vec![2, 1],
    );
    assert_ne!([255, 255, 255], li_merged_.0.colors[2]);
    assert_eq!(li_merged_, li_reference);
    let li_merged = li.merge(li_merged);
    let li_reference = LabelInfo {
        new_label: DEFAULT_LABEL.to_string(),
        labels: vec![
            "somelabel".to_string(),
            "new_label".to_string(),
            DEFAULT_LABEL.to_string(),
        ],
        colors: vec![[255, 255, 255], [0, 1, 1], li_merged.0.colors[2]],
        cat_ids: vec![1, 2, 3],
        cat_idx_current: 0,
        show_only_current: false,
    };
    assert_eq!(li_merged.0, li_reference);
}

#[test]
fn test_merge_annos() {
    let orig_shape = ShapeI::new(100, 100);
    let li1 = LabelInfo {
        new_label: "x".to_string(),
        labels: vec!["somelabel".to_string(), "x".to_string()],
        colors: vec![[255, 255, 255], [0, 1, 1]],
        cat_ids: vec![1, 2],
        cat_idx_current: 0,
        show_only_current: false,
    };
    let li2 = LabelInfo {
        new_label: "x".to_string(),
        labels: vec![
            "somelabel".to_string(),
            "new_label".to_string(),
            "x".to_string(),
        ],
        colors: vec![[255, 255, 255], [0, 1, 2], [1, 1, 1]],
        cat_ids: vec![1, 2, 3],
        cat_idx_current: 0,
        show_only_current: false,
    };
    let mut annos_map1: super::brush_data::BrushAnnoMap = AnnotationsMap::new();

    let mut line = Line::new();
    line.push(PtF { x: 5.0, y: 5.0 });
    let anno1 = Canvas::new(
        &BrushLine {
            line: line.clone(),
            thickness: 1.0,
            intensity: 1.0,
        },
        orig_shape,
        None,
    )
    .unwrap();
    annos_map1.insert(
        "file1".to_string(),
        (
            InstanceAnnotations::new(vec![anno1.clone()], vec![1], vec![true]).unwrap(),
            orig_shape,
        ),
    );
    let mut annos_map2: brush_data::BrushAnnoMap = AnnotationsMap::new();
    let anno2 = Canvas::new(
        &BrushLine {
            line,
            thickness: 2.0,
            intensity: 2.0,
        },
        orig_shape,
        None,
    )
    .unwrap();

    annos_map2.insert(
        "file1".to_string(),
        (
            InstanceAnnotations::new(vec![anno2.clone()], vec![1], vec![true]).unwrap(),
            orig_shape,
        ),
    );
    annos_map2.insert(
        "file2".to_string(),
        (
            InstanceAnnotations::new(vec![anno2.clone()], vec![1], vec![true]).unwrap(),
            orig_shape,
        ),
    );
    let (merged_map, merged_li) = merge(annos_map1, li1, annos_map2, li2.clone());
    let merged_li_ref = LabelInfo {
        new_label: "x".to_string(),
        labels: vec![
            "somelabel".to_string(),
            "x".to_string(),
            "new_label".to_string(),
        ],
        colors: vec![[255, 255, 255], [0, 1, 1], merged_li.colors[2]],
        cat_ids: vec![1, 2, 3],
        cat_idx_current: 0,
        show_only_current: false,
    };

    assert_eq!(merged_li, merged_li_ref);
    let map_ref = [
        (
            "file1".to_string(),
            (
                InstanceAnnotations::new(
                    vec![anno1, anno2.clone()],
                    vec![1, 2],
                    vec![false, false],
                )
                .unwrap(),
                orig_shape,
            ),
        ),
        (
            "file2".to_string(),
            (
                InstanceAnnotations::new(vec![anno2], vec![2], vec![false]).unwrap(),
                orig_shape,
            ),
        ),
    ]
    .into_iter()
    .collect::<AnnotationsMap<Canvas>>();
    for (k, (v, s)) in merged_map.iter() {
        assert_eq!(map_ref[k].0, *v);
        assert_eq!(map_ref[k].1, *s);
    }
}
