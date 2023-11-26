use std::{fmt::Display, ops::Range, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{core::max_squaredist, OutOfBoundsMode, PtF, PtI};
use crate::{
    result::{to_rv, RvError, RvResult},
    rverr, Shape,
};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BB {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
impl BB {
    /// `[x, y, w, h]`
    pub fn from_arr(a: &[u32; 4]) -> Self {
        BB {
            x: a[0],
            y: a[1],
            w: a[2],
            h: a[3],
        }
    }

    pub fn from_vec(points: &[PtI]) -> RvResult<Self> {
        let x_iter = points.iter().map(|p| p.x);
        let y_iter = points.iter().map(|p| p.y);
        let min_x = x_iter
            .clone()
            .min()
            .ok_or_else(|| rverr!("empty polygon",))?;
        let min_y = y_iter
            .clone()
            .min()
            .ok_or_else(|| rverr!("empty polygon",))?;
        let max_x = x_iter.max().ok_or_else(|| rverr!("empty polygon",))?;
        let max_y = y_iter.max().ok_or_else(|| rverr!("empty polygon",))?;
        Ok(BB::from_points(
            (min_x, min_y).into(),
            (max_x, max_y).into(),
        ))
    }

    pub fn distance_to_boundary(&self, pos: PtF) -> f32 {
        let dx = (self.x as f32 - pos.x).abs();
        let dw = ((self.x + self.w) as f32 - pos.x).abs();
        let dy = (self.y as f32 - pos.y).abs();
        let dh = ((self.y + self.h) as f32 - pos.y).abs();
        dx.min(dw).min(dy).min(dh)
    }

    pub fn split_horizontally(&self, y: u32) -> (Self, Self) {
        let top = BB::from_arr(&[self.x, self.y, self.w, y - self.y]);
        let btm = BB::from_arr(&[self.x, y, self.w, self.y_max() - y]);
        (top, btm)
    }
    pub fn split_vertically(&self, x: u32) -> (Self, Self) {
        let left = BB::from_arr(&[self.x, self.y, x - self.x, self.h]);
        let right = BB::from_arr(&[x, self.y, self.x_max() - x, self.h]);
        (left, right)
    }

    pub fn from_shape(shape: Shape) -> Self {
        BB {
            x: 0,
            y: 0,
            w: shape.w,
            h: shape.h,
        }
    }

    pub fn y_max(&self) -> u32 {
        self.y + self.h
    }

    pub fn x_max(&self) -> u32 {
        self.x + self.w
    }

    pub fn intersect(self, other: BB) -> BB {
        BB::from_points(
            (self.x.max(other.x), self.y.max(other.y)).into(),
            (
                self.x_max().min(other.x_max()),
                self.y_max().min(other.y_max()),
            )
                .into(),
        )
    }

    pub fn intersect_or_self(&self, other: Option<BB>) -> BB {
        if let Some(other) = other {
            self.intersect(other)
        } else {
            *self
        }
    }

    /// Return points of greatest distance between self and other
    pub fn max_squaredist<'a>(
        &'a self,
        other: impl Iterator<Item = PtI> + 'a + Clone,
    ) -> (PtI, PtI, i64) {
        max_squaredist(self.points_iter(), other)
    }

    pub fn min_max(&self, axis: usize) -> (u32, u32) {
        if axis == 0 {
            (self.x, self.x + self.w)
        } else {
            (self.y, self.y + self.h)
        }
    }

    /// Iteration order of corners
    /// 0   3
    /// v   ʌ
    /// 1 > 2
    #[allow(clippy::needless_lifetimes)]
    pub fn points_iter<'a>(&'a self) -> impl Iterator<Item = PtI> + 'a + Clone {
        (0..4).map(|idx| self.corner(idx))
    }

    pub fn corner(&self, idx: usize) -> PtI {
        let (x, y, w, h) = (self.x, self.y, self.w, self.h);
        match idx {
            0 => (x, y).into(),
            1 => (x, y + h - 1).into(),
            2 => (x + w - 1, y + h - 1).into(),
            3 => (x + w - 1, y).into(),
            _ => panic!("bounding boxes only have 4, {idx} is out of bounds"),
        }
    }
    pub fn opposite_corner(&self, idx: usize) -> PtI {
        self.corner((idx + 2) % 4)
    }

    pub fn shape(&self) -> Shape {
        Shape {
            w: self.w,
            h: self.h,
        }
    }

    pub fn from_points(p1: PtI, p2: PtI) -> Self {
        let x_min = p1.x.min(p2.x);
        let y_min = p1.y.min(p2.y);
        let x_max = p1.x.max(p2.x);
        let y_max = p1.y.max(p2.y);
        Self {
            x: x_min,
            y: y_min,
            w: x_max - x_min + 1, // x_min and x_max are both contained in the bb
            h: y_max - y_min + 1,
        }
    }

    pub fn x_range(&self) -> Range<u32> {
        self.x..(self.x + self.w)
    }

    pub fn y_range(&self) -> Range<u32> {
        self.y..(self.y + self.h)
    }

    pub fn center_f(&self) -> (f32, f32) {
        (
            self.w as f32 * 0.5 + self.x as f32,
            self.h as f32 * 0.5 + self.y as f32,
        )
    }

    pub fn center(&self) -> PtI {
        (self.x + self.w / 2, self.y + self.h / 2).into()
    }

    pub fn min_usize(&self) -> (usize, usize) {
        (self.x as usize, self.y as usize)
    }

    pub fn max_usize(&self) -> (usize, usize) {
        ((self.x + self.w) as usize, (self.y + self.h) as usize)
    }

    pub fn min(&self) -> PtI {
        (self.x, self.y).into()
    }

    pub fn max(&self) -> PtI {
        (self.x + self.w, self.y + self.h).into()
    }

    pub fn follow_movement(
        &self,
        from: PtF,
        to: PtF,
        shape: Shape,
        oob_mode: OutOfBoundsMode,
    ) -> Option<Self> {
        let x_shift: i32 = (to.x - from.x) as i32;
        let y_shift: i32 = (to.y - from.y) as i32;
        self.translate(x_shift, y_shift, shape, oob_mode)
    }

    pub fn covers_y(&self, y: f32) -> bool {
        self.y_max() as f32 > y && self.y as f32 <= y
    }
    pub fn covers_x(&self, x: f32) -> bool {
        self.x_max() as f32 > x && self.x as f32 <= x
    }

    pub fn contains<P>(&self, p: P) -> bool
    where
        P: Into<PtF>,
    {
        let p = p.into();
        self.covers_x(p.x) && self.covers_y(p.y)
    }

    pub fn contains_bb(&self, other: BB) -> bool {
        self.contains(other.min()) && self.contains(other.max())
    }

    pub fn is_contained_in_image(&self, shape: Shape) -> bool {
        self.x + self.w <= shape.w && self.y + self.h <= shape.h
    }

    pub fn new_shape_checked(
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        orig_im_shape: Shape,
        mode: OutOfBoundsMode,
    ) -> Option<Self> {
        match mode {
            OutOfBoundsMode::Deny => {
                if x < 0 || y < 0 || w < 1 || h < 1 {
                    None
                } else {
                    let bb = Self {
                        x: x as u32,
                        y: y as u32,
                        w: w as u32,
                        h: h as u32,
                    };
                    if bb.is_contained_in_image(orig_im_shape) {
                        Some(bb)
                    } else {
                        None
                    }
                }
            }
            OutOfBoundsMode::Resize(min_bb_shape) => {
                let bb = Self {
                    x: x.min(orig_im_shape.w as i32 - min_bb_shape.w as i32).max(0) as u32,
                    y: y.min(orig_im_shape.h as i32 - min_bb_shape.h as i32).max(0) as u32,
                    w: ((w + x.min(0)) as u32).max(min_bb_shape.w),
                    h: ((h + y.min(0)) as u32).max(min_bb_shape.h),
                };
                let mut bb_resized = bb.intersect(BB::from_shape(orig_im_shape));
                bb_resized.w = bb_resized.w.max(min_bb_shape.w);
                bb_resized.h = bb_resized.h.max(min_bb_shape.h);
                Some(bb_resized)
            }
        }
    }

    pub fn translate(
        &self,
        x_shift: i32,
        y_shift: i32,
        shape: Shape,
        oob_mode: OutOfBoundsMode,
    ) -> Option<Self> {
        let x = self.x as i32 + x_shift;
        let y = self.y as i32 + y_shift;
        Self::new_shape_checked(x, y, self.w as i32, self.h as i32, shape, oob_mode)
    }

    pub fn new_fit_to_image(x: i32, y: i32, w: i32, h: i32, shape: Shape) -> BB {
        let clip = |var: i32, size_bx: i32, size_im: i32| {
            if var < 0 {
                let size_bx: i32 = size_bx + var;
                (0, size_bx.min(size_im))
            } else {
                (var, (size_bx + var).min(size_im) - var)
            }
        };
        let (x, w) = clip(x, w, shape.w as i32);
        let (y, h) = clip(y, h, shape.h as i32);

        BB::from_arr(&[x as u32, y as u32, w as u32, h as u32])
    }

    pub fn center_scale(&self, factor: f32, shape: Shape) -> Self {
        let x = self.x as f32;
        let y = self.y as f32;
        let w = self.w as f32;
        let h = self.h as f32;
        let (cx, cy) = (w * 0.5 + x, h * 0.5 + y);
        let topleft = (cx + factor * (x - cx), cy + factor * (y - cy));
        let btmright = (cx + factor * (x + w - cx), cy + factor * (y + h - cy));
        let (x_tl, y_tl) = topleft;
        let (x_br, y_br) = btmright;
        let w = (x_br - x_tl).round() as i32;
        let h = (y_br - y_tl).round() as i32;
        let x = x_tl.round() as i32;
        let y = y_tl.round() as i32;

        Self::new_fit_to_image(x, y, w, h, shape)
    }

    pub fn shift_max(&self, x_shift: i32, y_shift: i32, shape: Shape) -> Option<Self> {
        let (w, h) = (self.w as i32 + x_shift, self.h as i32 + y_shift);
        Self::new_shape_checked(
            self.x as i32,
            self.y as i32,
            w,
            h,
            shape,
            OutOfBoundsMode::Deny,
        )
    }

    pub fn shift_min(&self, x_shift: i32, y_shift: i32, shape: Shape) -> Option<Self> {
        let (x, y) = (self.x as i32 + x_shift, self.y as i32 + y_shift);
        let (w, h) = (self.w as i32 - x_shift, self.h as i32 - y_shift);
        Self::new_shape_checked(x, y, w, h, shape, OutOfBoundsMode::Deny)
    }

    pub fn has_overlap(&self, other: &BB) -> bool {
        if self.points_iter().any(|c| other.contains(c)) {
            true
        } else {
            other.points_iter().any(|c| self.contains(c))
        }
    }
}

impl Display for BB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bb_str = format!("[{}, {}, {} ,{}]", self.x, self.y, self.w, self.h);
        f.write_str(bb_str.as_str())
    }
}
impl FromStr for BB {
    type Err = RvError;
    fn from_str(s: &str) -> RvResult<Self> {
        let err_parse = rverr!("could not parse '{}' into a bounding box", s);
        let mut int_iter = s[1..(s.len() - 1)]
            .split(',')
            .map(|cse| cse.trim().parse::<u32>().map_err(to_rv));
        let x = int_iter.next().ok_or_else(|| err_parse.clone())??;
        let y = int_iter.next().ok_or_else(|| err_parse.clone())??;
        let w = int_iter.next().ok_or_else(|| err_parse.clone())??;
        let h = int_iter.next().ok_or(err_parse)??;
        Ok(BB { x, y, w, h })
    }
}
