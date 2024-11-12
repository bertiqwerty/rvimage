use std::{
    fmt::Display,
    ops::{Neg, Range},
    str::FromStr,
};

use serde::{Deserialize, Serialize};

use super::{
    core::{
        clamp_sub_zero, max_from_partial, max_squaredist, min_from_partial, CoordinateBox, Max,
        Min, Shape,
    },
    Calc, OutOfBoundsMode, Point, PtF, PtI, TPtF, TPtI, TPtS,
};
use crate::{
    result::{to_rv, RvError, RvResult},
    rverr, ShapeI,
};

pub type BbI = BB<TPtI>;
pub type BbS = BB<TPtS>;
pub type BbF = BB<TPtF>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BB<T> {
    pub x: T,
    pub y: T,
    pub w: T,
    pub h: T,
}

impl<T> BB<T>
where
    T: Calc + CoordinateBox,
{
    /// `[x, y, w, h]`
    pub fn from_arr(a: &[T; 4]) -> Self {
        BB {
            x: a[0],
            y: a[1],
            w: a[2],
            h: a[3],
        }
    }

    pub fn merge(&self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let x_max = self.x_max().max(other.x_max());
        let y_max = self.y_max().max(other.y_max());
        BB::from_points((x, y).into(), (x_max, y_max).into())
    }

    // TODO: replace Point<T> by &Point<T>
    pub fn from_points_iter(points: impl Iterator<Item = Point<T>> + Clone) -> RvResult<Self> {
        let x_iter = points.clone().map(|p| p.x);
        let y_iter = points.map(|p| p.y);
        let min_x = x_iter
            .clone()
            .min_by(min_from_partial)
            .ok_or_else(|| rverr!("empty iterator"))?;
        let min_y = y_iter
            .clone()
            .min_by(min_from_partial)
            .ok_or_else(|| rverr!("empty iterator"))?;
        let max_x = x_iter
            .max_by(max_from_partial)
            .ok_or_else(|| rverr!("empty iterator"))?;
        let max_y = y_iter
            .max_by(max_from_partial)
            .ok_or_else(|| rverr!("empty iterator"))?;
        Ok(BB::from_points(
            Point { x: min_x, y: min_y },
            Point { x: max_x, y: max_y },
        ))
    }
    pub fn from_vec(points: &[Point<T>]) -> RvResult<Self> {
        Self::from_points_iter(points.iter().copied())
    }

    pub fn distance_to_boundary(&self, pos: Point<T>) -> T {
        let dx = (self.x - pos.x).abs();
        let dw = ((self.x + self.w) - pos.x).abs();
        let dy = (self.y - pos.y).abs();
        let dh = ((self.y + self.h) - pos.y).abs();
        dx.min(dw).min(dy).min(dh)
    }

    pub fn split_horizontally(&self, y: T) -> (Self, Self) {
        let top = BB::from_arr(&[self.x, self.y, self.w, y - self.y]);
        let btm = BB::from_arr(&[self.x, y, self.w, self.y_max() - y]);
        (top, btm)
    }
    pub fn split_vertically(&self, x: T) -> (Self, Self) {
        let left = BB::from_arr(&[self.x, self.y, x - self.x, self.h]);
        let right = BB::from_arr(&[x, self.y, self.x_max() - x, self.h]);
        (left, right)
    }
    pub fn from_shape_int(shape: ShapeI) -> Self {
        BB {
            x: T::from(0),
            y: T::from(0),
            w: T::from(shape.w),
            h: T::from(shape.h),
        }
    }

    pub fn from_shape(shape: Shape<T>) -> Self {
        BB {
            x: T::from(0),
            y: T::from(0),
            w: shape.w,
            h: shape.h,
        }
    }

    pub fn y_max(&self) -> T {
        // y_max is still part of the box, hence -1
        self.y + self.h - T::size_addon()
    }

    pub fn x_max(&self) -> T {
        // x_max is still part of the box, hence -1
        self.x + self.w - T::size_addon()
    }

    pub fn intersect(self, other: BB<T>) -> BB<T> {
        BB::from_points(
            Point {
                x: self.x.max(other.x),
                y: self.y.max(other.y),
            },
            Point {
                x: self.x_max().min(other.x_max()),
                y: self.y_max().min(other.y_max()),
            },
        )
    }

    pub fn points(&self) -> [Point<T>; 4] {
        [
            self.corner(0),
            self.corner(1),
            self.corner(2),
            self.corner(3),
        ]
    }

    pub fn intersect_or_self(&self, other: Option<BB<T>>) -> BB<T> {
        if let Some(other) = other {
            self.intersect(other)
        } else {
            *self
        }
    }

    /// Return points of greatest distance between self and other
    pub fn max_squaredist<'a>(
        &'a self,
        other: impl Iterator<Item = Point<T>> + 'a + Clone,
    ) -> (Point<T>, Point<T>, T) {
        max_squaredist(self.points_iter(), other)
    }

    pub fn min_max(&self, axis: usize) -> (T, T) {
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
    pub fn points_iter<'a>(&'a self) -> impl Iterator<Item = Point<T>> + 'a + Clone {
        (0..4).map(|idx| self.corner(idx))
    }

    pub fn corner(&self, idx: usize) -> Point<T> {
        let (x, y, w, h) = (self.x, self.y, self.w, self.h);
        match idx {
            0 => Point { x, y },
            1 => Point {
                x,
                y: y + h - T::size_addon(),
            },
            2 => (x + w - T::size_addon(), y + h - T::size_addon()).into(),
            3 => (x + w - T::size_addon(), y).into(),
            _ => panic!("bounding boxes only have 4, {idx} is out of bounds"),
        }
    }
    pub fn opposite_corner(&self, idx: usize) -> Point<T> {
        self.corner((idx + 2) % 4)
    }

    pub fn shape(&self) -> Shape<T> {
        Shape {
            w: self.w,
            h: self.h,
        }
    }

    pub fn from_points(p1: Point<T>, p2: Point<T>) -> Self {
        let x_min = p1.x.min(p2.x);
        let y_min = p1.y.min(p2.y);
        let x_max = p1.x.max(p2.x);
        let y_max = p1.y.max(p2.y);
        Self {
            x: x_min,
            y: y_min,
            w: x_max - x_min + T::size_addon(), // x_min and x_max are both contained in the bb
            h: y_max - y_min + T::size_addon(),
        }
    }

    pub fn x_range(&self) -> Range<T> {
        self.x..(self.x + self.w)
    }

    pub fn y_range(&self) -> Range<T> {
        self.y..(self.y + self.h)
    }

    pub fn center_f(&self) -> (f64, f64)
    where
        T: Into<f64>,
    {
        (
            self.w.into() * 0.5 + self.x.into(),
            self.h.into() * 0.5 + self.y.into(),
        )
    }

    pub fn min(&self) -> Point<T> {
        Point {
            x: self.x,
            y: self.y,
        }
    }

    pub fn max(&self) -> Point<T> {
        Point {
            x: self.x_max(),
            y: self.y_max(),
        }
    }

    pub fn covers_y(&self, y: T) -> bool {
        self.y_max() >= y && self.y <= y
    }
    pub fn covers_x(&self, x: T) -> bool {
        self.x_max() >= x && self.x <= x
    }

    pub fn contains<P>(&self, p: P) -> bool
    where
        P: Into<Point<T>>,
    {
        let p = p.into();
        self.covers_x(p.x) && self.covers_y(p.y)
    }

    pub fn contains_bb(&self, other: Self) -> bool {
        self.contains(other.min()) && self.contains(other.max())
    }

    pub fn is_contained_in_image(&self, shape: ShapeI) -> bool {
        self.x + self.w <= shape.w.into() && self.y + self.h <= shape.h.into()
    }

    pub fn new_shape_checked(
        x: T,
        y: T,
        w: T,
        h: T,
        orig_im_shape: ShapeI,
        mode: OutOfBoundsMode<T>,
    ) -> Option<Self> {
        match mode {
            OutOfBoundsMode::Deny => {
                if x < T::zero() || y < T::zero() || w < T::one() || h < T::one() {
                    None
                } else {
                    let bb = Self { x, y, w, h };
                    if bb.is_contained_in_image(orig_im_shape) {
                        Some(bb)
                    } else {
                        None
                    }
                }
            }
            OutOfBoundsMode::Resize(min_bb_shape) => {
                let bb = Self {
                    x: x.min(clamp_sub_zero(orig_im_shape.w.into(), min_bb_shape.w)),
                    y: y.min(clamp_sub_zero(orig_im_shape.h.into(), min_bb_shape.h)),
                    w: (w + x.min(T::zero())).max(min_bb_shape.w),
                    h: (h + y.min(T::zero())).max(min_bb_shape.h),
                };
                let mut bb_resized = bb.intersect(BB::from_shape_int(orig_im_shape));
                bb_resized.w = bb_resized.w.max(min_bb_shape.w);
                bb_resized.h = bb_resized.h.max(min_bb_shape.h);
                Some(bb_resized)
            }
        }
    }

    pub fn has_overlap(&self, other: &Self) -> bool {
        if self.points_iter().any(|c| other.contains(c)) {
            true
        } else {
            other.points_iter().any(|c| self.contains(c))
        }
    }

    pub fn rot90_with_image_ntimes(&self, shape: &ShapeI, n: u8) -> Self
    where
        T: Neg<Output = T>,
    {
        let p_min = self.min().rot90_with_image_ntimes(shape, n);
        let p_max = self.max().rot90_with_image_ntimes(shape, n);
        Self::from_points(p_min, p_max)
    }
}

impl BbF {
    pub fn translate(
        self,
        x_shift: f64,
        y_shift: f64,
        shape: ShapeI,
        oob_mode: OutOfBoundsMode<f64>,
    ) -> Option<Self> {
        let x = self.x + x_shift;
        let y = self.y + y_shift;
        Self::new_shape_checked(x, y, self.w, self.h, shape, oob_mode)
    }
    pub fn follow_movement(
        &self,
        from: PtF,
        to: PtF,
        shape: ShapeI,
        oob_mode: OutOfBoundsMode<f64>,
    ) -> Option<Self> {
        let x_shift = to.x - from.x;
        let y_shift = to.y - from.y;
        self.translate(x_shift, y_shift, shape, oob_mode)
    }

    pub fn new_fit_to_image(x: f64, y: f64, w: f64, h: f64, shape: ShapeI) -> Self {
        let clip = |var: f64, size_bx: f64, size_im: f64| {
            if var < 0.0 {
                let size_bx = size_bx + var;
                (0.0, size_bx.min(size_im))
            } else {
                (var, (size_bx + var).min(size_im) - var)
            }
        };
        let (x, w) = clip(x, w, shape.w.into());
        let (y, h) = clip(y, h, shape.h.into());

        Self::from_arr(&[x, y, w, h])
    }

    pub fn center_scale(&self, x_factor: f64, y_factor: f64, shape: ShapeI, center: Option<PtF>) -> Self {
        let x = self.x;
        let y = self.y;
        let w = self.w;
        let h = self.h;
        let c = center.unwrap_or(PtF{x: w * 0.5 + x,y: h * 0.5 + y});
        let topleft = (c.x + x_factor * (x - c.x), c.y + y_factor * (y - c.y));
        let btmright = (c.x + x_factor * (x + w - c.x), c.y + y_factor * (y + h - c.y));
        let (x_tl, y_tl) = topleft;
        let (x_br, y_br) = btmright;
        let w = x_br - x_tl;
        let h = y_br - y_tl;
        let x = x_tl.round();
        let y = y_tl.round();

        Self::new_fit_to_image(x, y, w, h, shape)
    }

    pub fn shift_max(&self, x_shift: f64, y_shift: f64, shape: ShapeI) -> Option<Self> {
        let (w, h) = (self.w + x_shift, self.h + y_shift);
        Self::new_shape_checked(self.x, self.y, w, h, shape, OutOfBoundsMode::Deny)
    }

    pub fn shift_min(&self, x_shift: f64, y_shift: f64, shape: ShapeI) -> Option<Self> {
        let (x, y) = (self.x + x_shift, self.y + y_shift);
        let (w, h) = (self.w - x_shift, self.h - y_shift);
        Self::new_shape_checked(x, y, w, h, shape, OutOfBoundsMode::Deny)
    }

    pub fn all_corners_close(&self, other: BbF) -> bool {
        fn close_floats(a: f64, b: f64) -> bool {
            (a - b).abs() < 1e-8
        }
        close_floats(self.x, other.x)
            && close_floats(self.y, other.y)
            && close_floats(self.w, other.w)
            && close_floats(self.h, other.h)
    }
}

impl From<BbF> for BbI {
    fn from(box_f: BbF) -> Self {
        let p_min: PtI = box_f.min().into();
        let p_max: PtI = box_f.max().into();
        let x = p_min.x;
        let y = p_min.y;
        let x_max = p_max.x - TPtI::size_addon();
        let y_max = p_max.y - TPtI::size_addon();
        BbI::from_points((x, y).into(), (x_max, y_max).into())
    }
}
impl From<BbI> for BbF {
    fn from(box_int: BbI) -> Self {
        let x = box_int.min().x;
        let y = box_int.min().y;
        let x_max = box_int.max().x + TPtI::size_addon();
        let y_max = box_int.max().y + TPtI::size_addon();
        BbF::from_points((x, y).into(), (x_max, y_max).into())
    }
}

impl From<BbI> for BbS {
    fn from(bb: BbI) -> Self {
        BbS::from_points(bb.min().into(), bb.max().into())
    }
}
impl From<BbS> for BbI {
    fn from(bb: BbS) -> Self {
        BbI::from_points(bb.min().into(), bb.max().into())
    }
}

impl BbI {
    pub fn expand(&self, x_expand: TPtI, y_expand: TPtI, shape: ShapeI) -> Self {
        let (x, y) = (
            self.x.saturating_sub(x_expand),
            self.y.saturating_sub(y_expand),
        );
        let (w, h) = (self.w + 2 * x_expand, self.h + 2 * y_expand);
        let (w, h) = (w.clamp(1, shape.w), h.clamp(1, shape.h));
        Self { x, y, w, h }
    }
}

impl<T> From<&[T; 4]> for BB<T>
where
    T: Calc + CoordinateBox,
{
    fn from(a: &[T; 4]) -> Self {
        Self::from_arr(a)
    }
}

impl Display for BbI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bb_str = format!("[{}, {}, {} ,{}]", self.x, self.y, self.w, self.h);
        f.write_str(bb_str.as_str())
    }
}
impl FromStr for BbI {
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
        Ok(BbI { x, y, w, h })
    }
}

#[cfg(test)]
use crate::PtS;

#[test]
fn test_rot() {
    let shape = &Shape::new(150, 123);
    let p_min = PtS { x: 1, y: 3 };
    let p_max = PtS { x: 6, y: 15 };
    let bb = BB::from_points(p_min, p_max);
    for n in 0..6 {
        let b_rotated = BB::from_points(
            p_min.rot90_with_image_ntimes(shape, n),
            p_max.rot90_with_image_ntimes(shape, n),
        );
        assert_eq!(b_rotated, bb.rot90_with_image_ntimes(shape, n));
    }
    let shape = &Shape::new(5, 10);
    let p_min = PtF { x: 1.0, y: 2.0 };
    let p_max = PtF { x: 2.0, y: 4.0 };
    let bb = BB::from_points(p_min, p_max);
    let p_min = PtF { x: 2.0, y: 4.0 };
    let p_max = PtF { x: 4.0, y: 3.0 };
    let bb_ref_1 = BB::from_points(p_min, p_max);
    assert_eq!(bb.rot90_with_image_ntimes(shape, 1), bb_ref_1);
}

#[test]
fn test_expand() {
    let bb = BbI::from_arr(&[0, 0, 10, 10]).expand(1, 1, Shape::new(10, 10));
    assert_eq!(bb, BbI::from_arr(&[0, 0, 10, 10]));

    let bb = BbI::from_arr(&[5, 5, 10, 10]).expand(1, 2, Shape::new(20, 20));
    assert_eq!(bb, BbI::from_arr(&[4, 3, 12, 14]));
}
