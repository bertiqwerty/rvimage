use std::collections::HashMap;
use std::ops::Index;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{cfg::ExportPath, util::Visibility, ShapeI};
use rvimage_domain::{rverr, RvResult};
use rvimage_domain::{BbF, PtF, TPtF, TPtI};

use super::annotations::InstanceAnnotations;

pub const OUTLINE_THICKNESS_CONVERSION: TPtF = 10.0;

const DEFAULT_LABEL: &str = "foreground";

fn color_dist(c1: [u8; 3], c2: [u8; 3]) -> f32 {
    let square_d = |i| (c1[i] as f32 - c2[i] as f32).powi(2);
    (square_d(0) + square_d(1) + square_d(2)).sqrt()
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportMode {
    Merge,
    #[default]
    Replace,
}
// pub type AnnotationsMap<T> = HashMap<String, (InstanceAnnotations<T>, ShapeI)>;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct AnnotationsMap<T> {
    #[serde(flatten)]
    map: HashMap<String, (InstanceAnnotations<T>, ShapeI)>,
}

impl<T> IntoIterator for AnnotationsMap<T> {
    type Item = (String, (InstanceAnnotations<T>, ShapeI));
    type IntoIter = std::collections::hash_map::IntoIter<String, (InstanceAnnotations<T>, ShapeI)>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.into_iter()
    }
}
impl<T> FromIterator<(String, (InstanceAnnotations<T>, ShapeI))> for AnnotationsMap<T> {
    fn from_iter<I: IntoIterator<Item = (String, (InstanceAnnotations<T>, ShapeI))>>(
        iter: I,
    ) -> Self {
        Self {
            map: iter.into_iter().collect(),
        }
    }
}
impl<T> Index<&str> for AnnotationsMap<T> {
    type Output = (InstanceAnnotations<T>, ShapeI);

    fn index(&self, index: &str) -> &Self::Output {
        &self.map[index]
    }
}
impl<T> AnnotationsMap<T> {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    pub fn insert(&mut self, key: String, value: (InstanceAnnotations<T>, ShapeI)) {
        self.map.insert(key, value);
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut (InstanceAnnotations<T>, ShapeI)> {
        self.map.get_mut(key)
    }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &(InstanceAnnotations<T>, ShapeI))> {
        self.map.iter()
    }
    pub fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (&String, &mut (InstanceAnnotations<T>, ShapeI))> {
        self.map.iter_mut()
    }
    pub fn get(&self, key: &str) -> Option<&(InstanceAnnotations<T>, ShapeI)> {
        self.map.get(key)
    }
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&String, &mut (InstanceAnnotations<T>, ShapeI)) -> bool,
    {
        self.map.retain(f);
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut (InstanceAnnotations<T>, ShapeI)> {
        self.map.values_mut()
    }
    pub fn remove(&mut self, key: &str) {
        self.map.remove(key);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub visible: bool,
    pub is_colorchange_triggered: bool,
    pub is_redraw_annos_triggered: bool,
    pub is_export_triggered: bool,
    pub is_export_absolute: bool,
    pub is_import_triggered: bool,
    pub import_mode: ImportMode,
    pub is_history_update_triggered: bool,
    pub track_changes: bool,
    pub erase: bool,
    pub label_propagation: Option<usize>,
    pub label_deletion: Option<usize>,
    pub auto_paste: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            visible: true,
            is_colorchange_triggered: false,
            is_redraw_annos_triggered: false,
            is_export_triggered: false,
            is_export_absolute: false,
            is_import_triggered: false,
            import_mode: ImportMode::default(),
            is_history_update_triggered: false,
            track_changes: false,
            erase: false,
            label_propagation: None,
            label_deletion: None,
            auto_paste: false,
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
        for show in self.show_mask.iter_mut() {
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

    for (k, (v2, s)) in annos2.into_iter() {
        if let Some((v1, _)) = annotations_map.get_mut(&k) {
            let (elts, cat_idxs, _) = v2.separate_data();
            v1.extend(
                elts.into_iter(),
                cat_idxs.into_iter().map(|old_idx| idx_map[old_idx]),
                s,
            );
            v1.deselect_all();
        } else {
            let (elts, cat_idxs, _) = v2.separate_data();
            let cat_idxs = cat_idxs
                .into_iter()
                .map(|old_idx| idx_map[old_idx])
                .collect::<Vec<_>>();
            let v2 = InstanceAnnotations::new_relaxed(elts, cat_idxs);
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
    /// Merges two LabelInfos. Returns the merged LabelInfo and a vector that maps
    /// the indices of the second LabelInfo to the indices of the merged LabelInfo.
    pub fn merge(mut self, other: Self) -> (Self, Vec<usize>) {
        let mut idx_map = vec![];
        for other_label in other.labels.into_iter() {
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
    pub fn from_iter(it: impl Iterator<Item = ((String, [u8; 3]), u32)>) -> RvResult<Self> {
        let mut info = Self::empty();
        for ((label, color), cat_id) in it {
            info.push(label, Some(color), Some(cat_id))?
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
pub struct InstanceExportData<A>
where
    A: InstanceAnnotate,
{
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

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CocoSegmentation {
    Polygon(Vec<Vec<TPtF>>),
    Rle(CocoRle),
}

pub trait InstanceAnnotate: Clone + Default + PartialEq {
    fn is_contained_in_image(&self, shape: ShapeI) -> bool;
    fn contains<P>(&self, point: P) -> bool
    where
        P: Into<PtF>;
    fn dist_to_boundary(&self, p: PtF) -> TPtF;
    fn rot90_with_image_ntimes(self, shape: &ShapeI, n: u8) -> RvResult<Self>;
    fn enclosing_bb(&self) -> BbF;
    fn to_cocoseg(
        &self,
        shape_im: ShapeI,
        is_export_absolute: bool,
    ) -> RvResult<Option<CocoSegmentation>>;
}
pub trait ExportAsCoco<A>
where
    A: InstanceAnnotate + 'static,
{
    fn separate_data(self) -> (Options, LabelInfo, AnnotationsMap<A>, ExportPath);
    fn cocofile_conn(&self) -> ExportPath;
    fn label_info(&self) -> &LabelInfo;
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
use rvimage_domain::{BrushLine, Canvas, Line};
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
        new_label: "foreground".to_string(),
        labels: vec!["foreground".to_string(), "new_label".to_string()],
        colors: vec![[255, 255, 255], [0, 0, 1]],
        cat_ids: vec![1, 2],
        cat_idx_current: 0,
        show_only_current: false,
    };
    assert_eq!(li_merged, li_reference);
    assert_eq!(li_merged.clone().merge(li_merged.clone()).0, li_reference);
    let li = LabelInfo {
        new_label: "foreground".to_string(),
        labels: vec!["somelabel".to_string(), "new_label".to_string()],
        colors: vec![[255, 255, 255], [0, 1, 1]],
        cat_ids: vec![1, 2],
        cat_idx_current: 0,
        show_only_current: false,
    };
    let li_merged_ = li_merged.clone().merge(li.clone());
    let li_reference = (
        LabelInfo {
            new_label: "foreground".to_string(),
            labels: vec![
                "foreground".to_string(),
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
        new_label: "foreground".to_string(),
        labels: vec![
            "somelabel".to_string(),
            "new_label".to_string(),
            "foreground".to_string(),
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
