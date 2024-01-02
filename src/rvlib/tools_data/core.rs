use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::{
    domain::{Annotate, TPtF},
    result::RvResult,
    rverr, ShapeI,
};

use super::annotations::InstanceAnnotations;

pub const OUTLINE_THICKNESS_CONVERSION: TPtF = 10.0;

const DEFAULT_LABEL: &str = "foreground";

fn color_dist(c1: [u8; 3], c2: [u8; 3]) -> f32 {
    let square_d = |i| (c1[i] as f32 - c2[i] as f32).powi(2);
    (square_d(0) + square_d(1) + square_d(2)).sqrt()
}

pub type AnnotationsMap<T> = HashMap<String, (InstanceAnnotations<T>, ShapeI)>;
#[derive(Clone, Copy, Deserialize, Serialize, Debug, PartialEq, Eq)]
pub struct Options {
    pub visible: bool,
    pub is_colorchange_triggered: bool,
    pub is_redraw_annos_triggered: bool,
    pub is_export_triggered: bool,
    pub is_history_update_triggered: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            visible: true,
            is_colorchange_triggered: false,
            is_redraw_annos_triggered: false,
            is_export_triggered: false,
            is_history_update_triggered: false,
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
pub struct LabelInfo {
    pub new_label: String,
    labels: Vec<String>,
    colors: Vec<[u8; 3]>,
    cat_ids: Vec<u32>,
    pub cat_idx_current: usize,
}
impl LabelInfo {
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
        }
    }
    pub fn remove_catidx<'a, T>(
        &mut self,
        cat_idx: usize,
        annotaions_map: &mut HashMap<String, (InstanceAnnotations<T>, ShapeI)>,
    ) where
        T: Annotate + PartialEq + Default + 'a,
    {
        if self.len() > 1 {
            self.remove(cat_idx);
            if self.cat_idx_current >= cat_idx.max(1) {
                self.cat_idx_current -= 1;
            }
            for (anno, _) in annotaions_map.values_mut() {
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
