use std::collections::HashMap;

use tinyvec::{tiny_vec, TinyVec};

use crate::{annotations::BboxAnnotations, implement_annotations_getters};

pub const N_LABELS_ON_STACK: usize = 24;
type LabelsVec = TinyVec<[String; N_LABELS_ON_STACK]>;
type ColorsVec = TinyVec<[[u8; 3]; N_LABELS_ON_STACK]>;

fn color_dist(c1: [u8; 3], c2: [u8; 3]) -> f32 {
    let square_d = |i| (c1[i] as f32 - c2[i] as f32).powi(2);
    (square_d(0) + square_d(1) + square_d(2)).sqrt()
}

fn random_clr() -> [u8; 3] {
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

fn new_color(colors: &[[u8; 3]]) -> [u8; 3] {
    let mut new_clr_proposals = [[0u8, 0u8, 0u8]; 10];
    for new_clr in &mut new_clr_proposals {
        *new_clr = random_clr();
    }
    argmax_clr_dist(&new_clr_proposals, colors)
}

static DEFAULT_BBOX_ANNOTATION: BboxAnnotations = BboxAnnotations::new();
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BboxSpecifics {
    pub new_label: String,
    labels: LabelsVec,
    colors: ColorsVec,
    pub cat_id_current: usize,
    // filename -> annotations per file
    annotations_map: HashMap<String, BboxAnnotations>,
}
impl BboxSpecifics {
    implement_annotations_getters!(&DEFAULT_BBOX_ANNOTATION, BboxAnnotations);
    pub fn remove_cat(&mut self, cat_id: usize) {
        if self.labels.len() > 1 {
            self.labels.remove(cat_id);
            self.colors.remove(cat_id);
            if self.cat_id_current >= cat_id.max(1) {
                self.cat_id_current -= 1;
            }
            for anno in self.annotations_map.values_mut() {
                anno.remove_cat(cat_id);
            }
        }
    }
    pub fn len(&self) -> usize {
        self.colors.len()
    }
    pub fn push(&mut self, label: String, color: Option<[u8; 3]>) {
        if let Some(idx) = self.labels.iter().position(|lab| lab == &label) {
            if let Some(clr) = color {
                self.colors[idx] = clr;
            }
        } else {
            self.labels.push(label);
            if let Some(clr) = color {
                self.colors.push(clr);
            } else {
                let new_clr = new_color(&self.colors);
                self.colors.push(new_clr);
            }
        }
    }
    pub fn colors(&self) -> &[[u8; 3]] {
        &self.colors
    }
    pub fn labels(&self) -> &[String] {
        &self.labels
    }
    fn new() -> Self {
        let new_label = "".to_string();
        let new_color = [255, 255, 255];
        let labels = tiny_vec!([String; N_LABELS_ON_STACK] => new_label.clone());
        let colors = tiny_vec!([[u8; 3]; N_LABELS_ON_STACK] => new_color);
        BboxSpecifics {
            new_label,
            labels,
            colors,
            cat_id_current: 0,
            annotations_map: HashMap::new(),
        }
    }
}
impl Default for BboxSpecifics {
    fn default() -> Self {
        Self::new()
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
