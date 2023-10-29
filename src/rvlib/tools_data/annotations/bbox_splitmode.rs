use serde::{Deserialize, Serialize};

use crate::{
    domain::{OutOfBoundsMode, Point, Shape, BB},
    util::true_indices,
};

use super::core::{resize_bbs, resize_bbs_inds};

fn resize_bbs_by_key(
    bbs: Vec<BB>,
    selected_bbs: &[bool],
    shiftee_key: impl Fn(&BB) -> u32,
    candidate_key: impl Fn(&BB) -> u32,
    resize: impl Fn(BB) -> Option<BB>,
) -> Vec<BB> {
    let indices = true_indices(selected_bbs);
    let opposite_shiftees = indices
        .flat_map(|shiftee_idx| {
            bbs.iter()
                .enumerate()
                .filter(|(_, t)| candidate_key(t) == shiftee_key(&bbs[shiftee_idx]))
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
    fn zero_direction(&self, x_shift: i32, y_shift: i32) -> (i32, i32) {
        match self {
            Self::Horizontal => (0, y_shift),
            Self::Vertical => (x_shift, 0),
            Self::None => (x_shift, y_shift),
        }
    }
    pub fn shift_min_bbs(
        &self,
        x_shift: i32,
        y_shift: i32,
        selected_bbs: &[bool],
        bbs: Vec<BB>,
        shape_orig: Shape,
    ) -> Vec<BB> {
        let (x_shift, y_shift) = self.zero_direction(x_shift, y_shift);
        let bbs = match self {
            SplitMode::Horizontal => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.y,
                |bb| bb.y_max(),
                |bb| bb.shift_max(x_shift, y_shift, shape_orig),
            ),
            SplitMode::Vertical => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.x,
                |bb| bb.x_max(),
                |bb| bb.shift_max(x_shift, y_shift, shape_orig),
            ),
            SplitMode::None => bbs,
        };
        resize_bbs(bbs, selected_bbs, |bb| {
            bb.shift_min(x_shift, y_shift, shape_orig)
        })
    }
    pub fn shift_max_bbs(
        &self,
        x_shift: i32,
        y_shift: i32,
        selected_bbs: &[bool],
        bbs: Vec<BB>,
        shape_orig: Shape,
    ) -> Vec<BB> {
        let (x_shift, y_shift) = self.zero_direction(x_shift, y_shift);

        let bbs = match self {
            SplitMode::Horizontal => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.y_max(),
                |bb| bb.y,
                |bb| bb.shift_min(x_shift, y_shift, shape_orig),
            ),
            SplitMode::Vertical => resize_bbs_by_key(
                bbs,
                selected_bbs,
                |bb| bb.x_max(),
                |bb| bb.x,
                |bb| bb.shift_min(x_shift, y_shift, shape_orig),
            ),
            SplitMode::None => bbs,
        };
        resize_bbs(bbs, selected_bbs, |bb| {
            bb.shift_max(x_shift, y_shift, shape_orig)
        })
    }
    pub fn bb_follow_movement(
        &self,
        bb: BB,
        mpo_from: Point,
        mpo_to: Point,
        orig_shape: Shape,
    ) -> (bool, BB) {
        match self {
            SplitMode::None => {
                let oob_mode = OutOfBoundsMode::Deny;
                if let Some(bb_moved) = bb.follow_movement(mpo_from, mpo_to, orig_shape, oob_mode) {
                    (true, bb_moved)
                } else {
                    (false, bb)
                }
            }
            SplitMode::Horizontal => {
                let mpo_to: Point = (mpo_from.x, mpo_to.y).into();
                let min_shape = Shape::new(1, 30);
                let oob_mode = OutOfBoundsMode::Resize(min_shape);
                let y_shift = mpo_to.y as i32 - mpo_from.y as i32;
                if y_shift > 0 && bb.y == 0 {
                    if let Some(bb_shifted) = bb.shift_max(0, y_shift, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, bb)
                    }
                } else if y_shift < 0 && bb.y + bb.h == orig_shape.h {
                    if let Some(bb_shifted) = bb.shift_min(0, y_shift, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, bb)
                    }
                } else if let Some(bb_moved) =
                    bb.follow_movement(mpo_from, mpo_to, orig_shape, oob_mode)
                {
                    (true, bb_moved)
                } else {
                    (false, bb)
                }
            }
            SplitMode::Vertical => {
                let mpo_to: Point = (mpo_to.x, mpo_from.y).into();
                let min_shape = Shape::new(30, 1);
                let oob_mode = OutOfBoundsMode::Resize(min_shape);
                let x_shift = mpo_to.x as i32 - mpo_from.x as i32;
                if x_shift > 0 && bb.x == 0 {
                    if let Some(bb_shifted) = bb.shift_max(x_shift, 0, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, bb)
                    }
                } else if x_shift < 0 && bb.x + bb.w == orig_shape.h {
                    if let Some(bb_shifted) = bb.shift_min(x_shift, 0, orig_shape) {
                        (true, bb_shifted)
                    } else {
                        (false, bb)
                    }
                } else if let Some(bb_moved) =
                    bb.follow_movement(mpo_from, mpo_to, orig_shape, oob_mode)
                {
                    (true, bb_moved)
                } else {
                    (false, bb)
                }
            }
        }
    }
}
