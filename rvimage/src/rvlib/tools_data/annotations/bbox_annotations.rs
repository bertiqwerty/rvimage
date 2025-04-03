use crate::{GeoFig, InstanceLabelDisplay};
use rvimage_domain::{BbF, OutOfBoundsMode, PtF, RvResult, ShapeI, TPtF};
use std::mem;

use super::{bbox_splitmode::SplitMode, core::InstanceAnnotations};

fn shift(
    mut geos: Vec<GeoFig>,
    selected_geo: &[bool],
    x_shift: f64,
    y_shift: f64,
    shape_orig: ShapeI,
    mut shift_bbs: impl FnMut(f64, f64, &[bool], Vec<BbF>, ShapeI) -> Vec<BbF>,
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
            GeoFig::Poly(_) => {
                if *is_selected {
                    selected_others_indices.push(idx);
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

pub type BboxAnnotations = InstanceAnnotations<GeoFig>;

impl BboxAnnotations {
    pub fn from_bbs(bbs: &[BbF], cat_id: usize) -> RvResult<BboxAnnotations> {
        let bbs_len = bbs.len();
        let elts = bbs.iter().map(|bb| GeoFig::BB(*bb)).collect();
        BboxAnnotations::new(elts, vec![cat_id; bbs_len], vec![false; bbs_len])
    }
    pub fn shift(
        self,
        x_shift: TPtF,
        y_shift: TPtF,
        shape_orig: ShapeI,
        split_mode: SplitMode,
    ) -> Self {
        let (elts, cat_idxs, selected_mask) = self.separate_data();
        let elts = shift(
            elts,
            &selected_mask,
            x_shift,
            y_shift,
            shape_orig,
            |x_shift, y_shift, selected_bbs, bbs, shape_orig| {
                let bbs = split_mode.shift_min_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig);
                split_mode.shift_max_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig)
            },
        );
        Self::new(elts, cat_idxs, selected_mask)
            .expect("after shift the number of elements cannot change")
    }
    pub fn shift_min_bbs(
        self,
        x_shift: TPtF,
        y_shift: TPtF,
        shape_orig: ShapeI,
        split_mode: SplitMode,
    ) -> Self {
        let (elts, cat_idxs, selected_mask) = self.separate_data();
        let elts = shift(
            elts,
            &selected_mask,
            x_shift,
            y_shift,
            shape_orig,
            |x_shift, y_shift, selected_bbs, bbs, shape_orig| {
                split_mode.shift_min_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig)
            },
        );
        Self::new(elts, cat_idxs, selected_mask)
            .expect("after shift the number of elements cannot change")
    }

    pub fn shift_max_bbs(
        self,
        x_shift: TPtF,
        y_shift: TPtF,
        shape_orig: ShapeI,
        split_mode: SplitMode,
    ) -> Self {
        let (elts, cat_idxs, selected_mask) = self.separate_data();
        let elts = shift(
            elts,
            &selected_mask,
            x_shift,
            y_shift,
            shape_orig,
            |x_shift, y_shift, selected_bbs, bbs, shape_orig| {
                split_mode.shift_max_bbs(x_shift, y_shift, selected_bbs, bbs, shape_orig)
            },
        );
        Self::new(elts, cat_idxs, selected_mask)
            .expect("after shift the number of elements cannot change")
    }

    pub fn add_bb(
        &mut self,
        bb: BbF,
        cat_idx: usize,
        instance_label_display: InstanceLabelDisplay,
    ) {
        self.add_elt(GeoFig::BB(bb), cat_idx, instance_label_display);
    }

    pub fn selected_follow_movement(
        self,
        mpo_from: PtF,
        mpo_to: PtF,
        orig_shape: ShapeI,
        split_mode: SplitMode,
    ) -> (Self, bool) {
        let mut moved_somebody = false;
        let (mut elts, cat_idxs, selected_mask) = self.separate_data();
        for (geo, is_bb_selected) in elts.iter_mut().zip(selected_mask.iter()) {
            if *is_bb_selected {
                (moved_somebody, *geo) =
                    split_mode.geo_follow_movement(mem::take(geo), mpo_from, mpo_to, orig_shape);
            }
        }
        let x = Self::new(elts, cat_idxs, selected_mask)
            .expect("after follow movement the number of elements cannot change");
        (x, moved_somebody)
    }
}
#[cfg(test)]
use super::core::resize_bbs;
#[cfg(test)]
fn make_test_bbs() -> Vec<BbF> {
    vec![
        BbF {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        },
        BbF {
            x: 5.0,
            y: 5.0,
            w: 10.0,
            h: 10.0,
        },
        BbF {
            x: 9.0,
            y: 9.0,
            w: 10.0,
            h: 10.0,
        },
    ]
}
#[test]
fn test_bbs() {
    let bbs = make_test_bbs();
    let shape_orig = ShapeI { w: 100, h: 100 };

    // shift max
    let resized = resize_bbs(bbs.clone(), &[false, true, true], |bb| {
        bb.shift_max(-1.0, 1.0, shape_orig)
    });
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(BbF::from(&[5.0, 5.0, 9.0, 11.0]), resized[1]);
    assert_eq!(BbF::from(&[9.0, 9.0, 9.0, 11.0]), resized[2]);

    // shift min
    let resized = resize_bbs(bbs.clone(), &[false, true, true], |bb| {
        bb.shift_min(-1.0, 1.0, shape_orig)
    });
    assert_eq!(resized[0], bbs[0]);
    assert_eq!(BbF::from(&[4.0, 6.0, 11.0, 9.0]), resized[1]);
    assert_eq!(BbF::from(&[8.0, 10.0, 11.0, 9.0]), resized[2]);
}
#[test]
fn test_annos() {
    fn len_check(annos: &BboxAnnotations) {
        assert_eq!(annos.selected_mask().len(), annos.elts().len());
        assert_eq!(annos.cat_idxs().len(), annos.elts().len());
    }
    let mut annos = BboxAnnotations::from_bbs(&make_test_bbs(), 0).unwrap();
    len_check(&annos);
    let idx = 1;
    assert!(!annos.selected_mask()[idx]);
    annos.select(idx);
    len_check(&annos);
    annos.label_selected(3);
    len_check(&annos);
    for i in 0..(annos.elts().len()) {
        if i == idx {
            assert_eq!(annos.cat_idxs()[i], 3);
        } else {
            assert_eq!(annos.cat_idxs()[i], 0);
        }
    }
    assert!(annos.selected_mask()[idx]);
    annos.deselect(idx);
    len_check(&annos);
    assert!(!annos.selected_mask()[idx]);
    annos.toggle_selection(idx);
    len_check(&annos);
    assert!(annos.selected_mask()[idx]);
    annos.remove_selected();
    len_check(&annos);
    assert!(annos.elts().len() == make_test_bbs().len() - 1);
    assert!(annos.selected_mask().len() == make_test_bbs().len() - 1);
    assert!(annos.cat_idxs().len() == make_test_bbs().len() - 1);
    // this time nothing should be removed
    annos.remove_selected();
    len_check(&annos);
    assert!(annos.elts().len() == make_test_bbs().len() - 1);
    assert!(annos.selected_mask().len() == make_test_bbs().len() - 1);
    assert!(annos.cat_idxs().len() == make_test_bbs().len() - 1);
    annos.remove(0);
    len_check(&annos);
    assert!(annos.elts().len() == make_test_bbs().len() - 2);
    assert!(annos.selected_mask().len() == make_test_bbs().len() - 2);
    assert!(annos.cat_idxs().len() == make_test_bbs().len() - 2);
    annos.add_bb(make_test_bbs()[0], 0, InstanceLabelDisplay::None);
    len_check(&annos);
    annos.add_bb(make_test_bbs()[0], 123, InstanceLabelDisplay::IndexLr);
    len_check(&annos);
    annos.clear();
    len_check(&annos);
    assert!(annos.elts().is_empty());
    assert!(annos.selected_mask().is_empty());
    assert!(annos.cat_idxs().is_empty());
    assert!(annos.cat_idxs().is_empty());
}
