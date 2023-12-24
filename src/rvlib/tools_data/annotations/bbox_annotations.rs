use crate::{
    domain::{OutOfBoundsMode, PtF, Shape, BB},
    util::true_indices,
    GeoFig,
};
use serde::{Deserialize, Serialize};
use std::mem;

use super::bbox_splitmode::SplitMode;

fn shift(
    mut geos: Vec<GeoFig>,
    selected_geo: &[bool],
    x_shift: i32,
    y_shift: i32,
    shape_orig: Shape,
    mut shift_bbs: impl FnMut(i32, i32, &[bool], Vec<BB>, Shape) -> Vec<BB>,
) -> Vec<GeoFig> {
    // Bounding boxes have a split-functionality. Hence, they are treated separately.
    let mut bb_indices = vec![];
    let mut selected_others_indices = vec![];
    let mut selected_bbs = vec![];
    let mut bbs = vec![];
    for (idx, (g, is_selected)) in geos.iter().zip(selected_geo.iter()).enumerate() {
        match g {
            GeoFig::BB(bb) => {
                bb_indices.push(idx);
                bbs.push(*bb);
                selected_bbs.push(*is_selected);
            }
            _ => {
                if *is_selected {
                    selected_others_indices.push(idx)
                }
            }
        }
    }
    let bbs = shift_bbs(
        x_shift,
        y_shift,
        &selected_bbs,
        mem::take(&mut bbs),
        shape_orig,
    );

    for oth_idx in selected_others_indices {
        if let Some(translated) = geos[oth_idx].clone().translate(
            (x_shift, y_shift).into(),
            shape_orig,
            OutOfBoundsMode::Deny,
        ) {
            geos[oth_idx] = translated;
        }
    }
    for (bb_idx, bb) in bb_indices.iter().zip(bbs.iter()) {
        geos[*bb_idx] = GeoFig::BB(*bb);
    }
    geos
}
#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BboxAnnotations {
    geos: Vec<GeoFig>,
    cat_idxs: Vec<usize>,
    selected_bbs: Vec<bool>,
}

impl BboxAnnotations {
    pub const fn new() -> Self {
        BboxAnnotations {
            geos: vec![],
            cat_idxs: vec![],
            selected_bbs: vec![],
        }
    }

    pub fn data(self) -> (Vec<GeoFig>, Vec<usize>) {
        (self.geos, self.cat_idxs)
    }

    pub fn extend<IG, IC>(&mut self, geos: IG, cat_ids: IC, shape_image: Shape)
    where
        IG: Iterator<Item = GeoFig>,
        IC: Iterator<Item = usize>,
    {
        for (geo, cat_id) in geos.zip(cat_ids) {
            if geo.is_contained_in_image(shape_image) && !self.geos().contains(&geo) {
                self.add_geo(geo, cat_id)
            }
        }
    }

    pub fn from_bbs_cats(geos: Vec<GeoFig>, cat_ids: Vec<usize>) -> BboxAnnotations {
        let geos_len = geos.len();
        BboxAnnotations {
            geos,
            cat_idxs: cat_ids,
            selected_bbs: vec![false; geos_len],
        }
    }

    pub fn from_bbs(bbs: Vec<BB>, cat_id: usize) -> BboxAnnotations {
        let bbs_len = bbs.len();
        let geos = bbs.iter().map(|bb| GeoFig::BB(*bb)).collect();
        BboxAnnotations {
            geos,
            cat_idxs: vec![cat_id; bbs_len],
            selected_bbs: vec![false; bbs_len],
        }
    }

    pub fn reduce_cat_idxs(&mut self, cat_idx: usize) {
        if cat_idx > 0 {
            for cid in self.cat_idxs.iter_mut() {
                if *cid >= cat_idx {
                    *cid -= 1;
                }
            }
        }
    }

    pub fn remove(&mut self, box_idx: usize) -> GeoFig {
        self.cat_idxs.remove(box_idx);
        self.selected_bbs.remove(box_idx);
        self.geos.remove(box_idx)
    }

    pub fn remove_multiple(&mut self, indices: &[usize]) {
        let keep_indices = (0..self.geos.len()).filter(|i| !indices.contains(i));
        self.geos = keep_indices
            .clone()
            .map(|i| mem::take(&mut self.geos[i]))
            .collect::<Vec<_>>();
        self.cat_idxs = keep_indices.map(|i| self.cat_idxs[i]).collect::<Vec<_>>();
        self.selected_bbs = vec![false; self.geos.len()];
    }

    pub fn remove_selected(&mut self) {
        let selected = true_indices(self.selected_bbs());
        self.remove_multiple(&selected.collect::<Vec<_>>());
    }

    pub fn shift(&mut self, x_shift: i32, y_shift: i32, shape_orig: Shape, split_mode: SplitMode) {
        self.geos = shift(
            mem::take(&mut self.geos),
            self.selected_bbs(),
            x_shift,
            y_shift,
            shape_orig,
            |x_shift, y_shift, selected_bbs, bbs, shape_orig| {
                let bbs = split_mode.shift_min_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig);
                split_mode.shift_max_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig)
            },
        );
    }
    pub fn shift_min_bbs(
        &mut self,
        x_shift: i32,
        y_shift: i32,
        shape_orig: Shape,
        split_mode: SplitMode,
    ) {
        self.geos = shift(
            mem::take(&mut self.geos),
            self.selected_bbs(),
            x_shift,
            y_shift,
            shape_orig,
            |x_shift, y_shift, selected_bbs, bbs, shape_orig| {
                split_mode.shift_min_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig)
            },
        );
    }

    pub fn shift_max_bbs(
        &mut self,
        x_shift: i32,
        y_shift: i32,
        shape_orig: Shape,
        split_mode: SplitMode,
    ) {
        self.geos = shift(
            mem::take(&mut self.geos),
            self.selected_bbs(),
            x_shift,
            y_shift,
            shape_orig,
            |x_shift, y_shift, selected_bbs, bbs, shape_orig| {
                split_mode.shift_max_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig)
            },
        );
    }

    pub fn add_geo(&mut self, geo: GeoFig, cat_idx: usize) {
        self.cat_idxs.push(cat_idx);
        self.geos.push(geo);
        self.selected_bbs.push(false);
    }
    pub fn add_bb(&mut self, bb: BB, cat_idx: usize) {
        self.cat_idxs.push(cat_idx);
        self.geos.push(GeoFig::BB(bb));
        self.selected_bbs.push(false);
    }

    pub fn cat_idxs(&self) -> &Vec<usize> {
        &self.cat_idxs
    }

    pub fn geos(&self) -> &Vec<GeoFig> {
        &self.geos
    }

    pub fn deselect(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = false;
    }

    pub fn deselect_all(&mut self) {
        for s in &mut self.selected_bbs {
            *s = false;
        }
    }

    pub fn toggle_selection(&mut self, box_idx: usize) {
        let is_selected = self.selected_bbs[box_idx];
        if is_selected {
            self.deselect(box_idx);
        } else {
            self.select(box_idx);
        }
    }

    pub fn select(&mut self, box_idx: usize) {
        self.selected_bbs[box_idx] = true;
    }

    pub fn select_multi(&mut self, box_idxs: impl Iterator<Item = usize>) {
        for box_idx in box_idxs {
            self.select(box_idx);
        }
    }

    pub fn select_all(&mut self) {
        let n_bbs = self.geos.len();
        self.select_multi(0..n_bbs);
    }

    pub fn select_last_n(&mut self, n: usize) {
        let len = self.geos.len();
        self.select_multi((len - n)..len);
    }

    pub fn selected_bbs(&self) -> &Vec<bool> {
        &self.selected_bbs
    }

    pub fn selected_follow_movement(
        &mut self,
        mpo_from: PtF,
        mpo_to: PtF,
        orig_shape: Shape,
        split_mode: SplitMode,
    ) -> bool {
        let mut moved_somebody = false;
        for (geo, is_bb_selected) in self.geos.iter_mut().zip(self.selected_bbs.iter()) {
            if *is_bb_selected {
                (moved_somebody, *geo) =
                    split_mode.geo_follow_movement(mem::take(geo), mpo_from, mpo_to, orig_shape)
            }
        }
        moved_somebody
    }

    pub fn label_selected(&mut self, cat_id: usize) {
        let selected_inds = true_indices(&self.selected_bbs);
        for idx in selected_inds {
            self.cat_idxs[idx] = cat_id;
        }
    }

    pub fn clear(&mut self) {
        self.geos.clear();
        self.selected_bbs.clear();
        self.cat_idxs.clear();
    }
}
#[cfg(test)]
use {super::core::resize_bbs, crate::point_i};
#[cfg(test)]
fn make_test_bbs() -> Vec<BB> {
    vec![
        BB {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        },
        BB {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
        },
        BB {
            x: 9,
            y: 9,
            w: 10,
            h: 10,
        },
    ]
}
#[test]
fn test_bbs() {
    let bbs = make_test_bbs();
    let shape_orig = Shape { w: 100, h: 100 };

    // shift max
    let resized = resize_bbs(bbs.clone(), &[false, true, true], |bb| {
        bb.shift_max(-1, 1, shape_orig)
    });
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(
        BB::from_points(point_i!(5, 5), point_i!(13, 15)),
        resized[1]
    );
    assert_eq!(
        BB::from_points(point_i!(9, 9), point_i!(17, 19)),
        resized[2]
    );

    // shift min
    let resized = resize_bbs(bbs.clone(), &[false, true, true], |bb| {
        bb.shift_min(-1, 1, shape_orig)
    });
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(
        BB::from_points(point_i!(4, 6), point_i!(14, 14)),
        resized[1]
    );
    assert_eq!(
        BB::from_points(point_i!(8, 10), point_i!(18, 18)),
        resized[2]
    );
}
#[test]
fn test_annos() {
    fn len_check(annos: &BboxAnnotations) {
        assert_eq!(annos.selected_bbs.len(), annos.geos.len());
        assert_eq!(annos.cat_idxs.len(), annos.geos.len());
    }
    let mut annos = BboxAnnotations::from_bbs(make_test_bbs(), 0);
    len_check(&annos);
    let idx = 1;
    assert!(!annos.selected_bbs[idx]);
    annos.select(idx);
    len_check(&annos);
    annos.label_selected(3);
    len_check(&annos);
    for i in 0..(annos.geos.len()) {
        if i == idx {
            assert_eq!(annos.cat_idxs[i], 3);
        } else {
            assert_eq!(annos.cat_idxs[i], 0);
        }
    }
    assert!(annos.selected_bbs[idx]);
    annos.deselect(idx);
    len_check(&annos);
    assert!(!annos.selected_bbs[idx]);
    annos.toggle_selection(idx);
    len_check(&annos);
    assert!(annos.selected_bbs[idx]);
    annos.remove_selected();
    len_check(&annos);
    assert!(annos.geos.len() == make_test_bbs().len() - 1);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.cat_idxs.len() == make_test_bbs().len() - 1);
    // this time nothing should be removed
    annos.remove_selected();
    len_check(&annos);
    assert!(annos.geos.len() == make_test_bbs().len() - 1);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 1);
    assert!(annos.cat_idxs.len() == make_test_bbs().len() - 1);
    annos.remove(0);
    len_check(&annos);
    assert!(annos.geos.len() == make_test_bbs().len() - 2);
    assert!(annos.selected_bbs.len() == make_test_bbs().len() - 2);
    assert!(annos.cat_idxs.len() == make_test_bbs().len() - 2);
    annos.add_bb(make_test_bbs()[0].clone(), 0);
    len_check(&annos);
    annos.add_bb(make_test_bbs()[0].clone(), 123);
    len_check(&annos);
    annos.clear();
    len_check(&annos);
    assert!(annos.geos.len() == 0);
    assert!(annos.selected_bbs.len() == 0);
    assert!(annos.cat_idxs.len() == 0);
    assert!(annos.cat_idxs.len() == 0);
}
