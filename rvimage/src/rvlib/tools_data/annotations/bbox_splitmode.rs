use serde::{Deserialize, Serialize};

use crate::{GeoFig, util::true_indices};
use rvimage_domain::{BbF, CoordinateBox, OutOfBoundsMode, PtF, ShapeF, ShapeI, TPtF};

use super::core::{resize_bbs, resize_bbs_inds};

fn resize_bbs_by_key(
    bbs: Vec<BbF>,
    selected_bbs: &[bool],
    shiftee_key: impl Fn(&BbF) -> TPtF,
    candidate_key: impl Fn(&BbF) -> TPtF,
    resize: impl Fn(BbF) -> Option<BbF>,
) -> Vec<BbF> {
    let indices = true_indices(selected_bbs);
    let opposite_shiftees = indices
        .flat_map(|shiftee_idx| {
            bbs.iter()
                .enumerate()
                .filter(|(_, t)| candidate_key(t).is_close_to(shiftee_key(&bbs[shiftee_idx])))
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    resize_bbs_inds(bbs, opposite_shiftees.into_iter(), resize)
}
#[derive(Deserialize, Serialize, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitMode {
    Horizontal,
    Vertical,
    #[default]
    None,
}
impl SplitMode {
    fn zero_direction(self, x_shift: f64, y_shift: f64) -> (f64, f64) {
        match self {
            Self::Horizontal => (0.0, y_shift),
            Self::Vertical => (x_shift, 0.0),
            Self::None => (x_shift, y_shift),
        }
    }
    pub fn shift_min_bbs(
        self,
        x_shift: f64,
        y_shift: f64,
        selected_bbs: &[bool],
        bbs: Vec<BbF>,
        shape_orig: ShapeI,
    ) -> Vec<BbF> {
        let (x_shift, y_shift) = self.zero_direction(x_shift, y_shift);
        let bbs = match self {
            SplitMode::Horizontal => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.y,
                |bb| bb.y_max() + TPtF::size_addon(),
                |bb| bb.shift_max(x_shift, y_shift, shape_orig),
            ),
            SplitMode::Vertical => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.x,
                |bb| bb.x_max() + TPtF::size_addon(),
                |bb| bb.shift_max(x_shift, y_shift, shape_orig),
            ),
            SplitMode::None => bbs,
        };
        resize_bbs(bbs, selected_bbs, |bb| {
            bb.shift_min(x_shift, y_shift, shape_orig)
        })
    }
    pub fn shift_max_bbs(
        self,
        x_shift: TPtF,
        y_shift: TPtF,
        selected_bbs: &[bool],
        bbs: Vec<BbF>,
        shape_orig: ShapeI,
    ) -> Vec<BbF> {
        let (x_shift, y_shift) = self.zero_direction(x_shift, y_shift);

        let bbs = match self {
            SplitMode::Horizontal => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.y_max() + TPtF::size_addon(),
                |bb| bb.y,
                |bb| bb.shift_min(x_shift, y_shift, shape_orig),
            ),
            SplitMode::Vertical => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.x_max() + TPtF::size_addon(),
                |bb| bb.x,
                |bb| bb.shift_min(x_shift, y_shift, shape_orig),
            ),
            SplitMode::None => bbs,
        };
        resize_bbs(bbs, selected_bbs, |bb| {
            bb.shift_max(x_shift, y_shift, shape_orig)
        })
    }
    pub fn geo_follow_movement(
        self,
        geo: GeoFig,
        mpo_from: PtF,
        mpo_to: PtF,
        orig_shape: ShapeI,
    ) -> (bool, GeoFig) {
        match (self, &geo) {
            (SplitMode::None, _) => {
                let oob_mode = OutOfBoundsMode::Deny;
                let (has_moved, geo) = if let Some(bb_moved) = geo
                    .clone()
                    .follow_movement(mpo_from, mpo_to, orig_shape, oob_mode)
                {
                    (true, bb_moved)
                } else {
                    (false, geo)
                };
                (has_moved, geo)
            }
            (SplitMode::Horizontal, GeoFig::BB(bb)) => {
                let mpo_to: PtF = (mpo_from.x, mpo_to.y).into();
                let min_shape = ShapeF::new(1.0, 30.0);
                let oob_mode = OutOfBoundsMode::Resize(min_shape);
                let y_shift = mpo_to.y - mpo_from.y;
                let (has_moved, bb) = if y_shift > 0.0 && bb.y.is_close_to(0.0) {
                    if let Some(bb_shifted) = bb.shift_max(0.0, y_shift, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, *bb)
                    }
                } else if y_shift < 0.0 && (bb.y + bb.h).is_close_to(orig_shape.h.into()) {
                    if let Some(bb_shifted) = bb.shift_min(0.0, y_shift, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, *bb)
                    }
                } else if let Some(bb_moved) =
                    bb.follow_movement(mpo_from, mpo_to, orig_shape, oob_mode)
                {
                    (true, bb_moved)
                } else {
                    (false, *bb)
                };
                (has_moved, GeoFig::BB(bb))
            }
            (SplitMode::Vertical, GeoFig::BB(bb)) => {
                let mpo_to: PtF = (mpo_to.x, mpo_from.y).into();
                let min_shape = ShapeF::new(30.0, 1.0);
                let oob_mode = OutOfBoundsMode::Resize(min_shape);
                let x_shift = mpo_to.x - mpo_from.x;
                let (has_moved, bb) = if x_shift > 0.0 && bb.x.is_close_to(0.0) {
                    if let Some(bb_shifted) = bb.shift_max(x_shift, 0.0, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, *bb)
                    }
                } else if x_shift < 0.0 && (bb.x + bb.w).is_close_to(orig_shape.h.into()) {
                    if let Some(bb_shifted) = bb.shift_min(x_shift, 0.0, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, *bb)
                    }
                } else if let Some(bb_moved) =
                    bb.follow_movement(mpo_from, mpo_to, orig_shape, oob_mode)
                {
                    (true, bb_moved)
                } else {
                    (false, *bb)
                };
                (has_moved, GeoFig::BB(bb))
            }
            _ => (false, geo),
        }
    }
}
#[test]
fn test() {
    let bbs = vec![
        BbF::from(&[0.0, 0.0, 10.0, 10.0]),
        BbF::from(&[0.0, 10.0, 10.0, 10.0]),
        BbF::from(&[0.0, 20.0, 10.0, 10.0]),
        BbF::from(&[0.0, 30.0, 10.0, 10.0]),
        BbF::from(&[0.0, 40.0, 10.0, 10.0]),
        BbF::from(&[0.0, 50.0, 10.0, 10.0]),
        BbF::from(&[0.0, 60.0, 10.0, 10.0]),
        BbF::from(&[0.0, 70.0, 10.0, 10.0]),
        BbF::from(&[0.0, 80.0, 10.0, 10.0]),
        BbF::from(&[0.0, 90.0, 10.0, 10.0]),
    ];
    let trues = vec![3];
    let mut selected_bbs = vec![false; bbs.len()];
    for t in trues {
        selected_bbs[t] = true;
    }
    let split_mode = SplitMode::Horizontal;
    let shape_orig = ShapeI::new(100, 100);
    let bbs_min_shifted =
        split_mode.shift_min_bbs(0.0, 1.0, &selected_bbs, bbs.clone(), shape_orig);
    let bbs_max_shifted =
        split_mode.shift_max_bbs(0.0, 1.0, &selected_bbs, bbs.clone(), shape_orig);
    for (i, (bb_mins, (bb, bb_maxs))) in (bbs_min_shifted
        .iter()
        .zip(bbs.iter().zip(bbs_max_shifted.iter())))
    .enumerate()
    {
        if selected_bbs[i] {
            // self is selected
            assert_eq!(bb_maxs.y, bb.y);
            assert_eq!(bb_maxs.y_max(), bb.y_max() + TPtF::from(1));
            assert_eq!(bb_mins.y, bb.y + TPtF::from(1));
            assert_eq!(bb_mins.y_max(), bb.y_max());
        } else if i < selected_bbs.len() - 1 && selected_bbs[i + 1] {
            // successor of self is selected
            assert_eq!(bb_maxs.y, bb.y);
            assert_eq!(bb_maxs.y_max(), bb.y_max());
            assert_eq!(bb_mins.y, bb.y);
            assert_eq!(bb_mins.y_max(), bb.y_max() + TPtF::from(1));
        } else if i > 0 && selected_bbs[i - 1] {
            // predecessor of self is selected
            assert_eq!(bb_mins.y, bb.y);
            assert_eq!(bb_mins.y_max(), bb.y_max());
            assert_eq!(bb_maxs.y, bb.y + TPtF::from(1));
            assert_eq!(bb_maxs.y_max(), bb.y_max());
        } else {
            assert_eq!(bb_maxs.y, bb.y);
            assert_eq!(bb_maxs.y_max(), bb.y_max());
            assert_eq!(bb_mins.y, bb.y);
            assert_eq!(bb_mins.y_max(), bb.y_max());
        }
    }
}
