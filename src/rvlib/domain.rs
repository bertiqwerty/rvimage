use image::GenericImage;
use serde::{Deserialize, Serialize};

use std::{
    fmt::Display,
    iter::{self, Flatten},
    ops::{Add, Div, Mul, Range, Sub},
    str::FromStr,
};

use crate::{
    result::{to_rv, RvError, RvResult},
    rverr,
};

pub trait Calc:
    Mul<Output = Self> + Div<Output = Self> + Add<Output = Self> + Sub<Output = Self>
where
    Self: Sized,
{
}
impl Calc for u32 {}
impl Calc for f32 {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape {
    pub w: u32,
    pub h: u32,
}
impl Shape {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }
    pub fn from_im<I>(im: &I) -> Self
    where
        I: GenericImage,
    {
        Self {
            w: im.width(),
            h: im.height(),
        }
    }
}

impl From<[usize; 2]> for Shape {
    fn from(value: [usize; 2]) -> Self {
        Self::new(value[0] as u32, value[1] as u32)
    }
}

/// shape of the image that fits into the window
pub fn shape_scaled(shape_unscaled: Shape, shape_win: Shape) -> (f32, f32) {
    let w_ratio = shape_unscaled.w as f32 / shape_win.w as f32;
    let h_ratio = shape_unscaled.h as f32 / shape_win.h as f32;
    let ratio = w_ratio.max(h_ratio);
    let w_new = shape_unscaled.w as f32 / ratio;
    let h_new = shape_unscaled.h as f32 / ratio;
    (w_new, h_new)
}
/// shape without scaling to window
pub fn shape_unscaled(zoom_box: &Option<BB>, shape_orig: Shape) -> Shape {
    zoom_box.map_or(shape_orig, |z| z.shape())
}

pub fn pos_transform<F>(
    pos: PtF,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
    transform: F,
) -> PtF
where
    F: Fn(f32, f32, f32, f32) -> f32,
{
    let unscaled = shape_unscaled(zoom_box, shape_orig);
    let (w_scaled, h_scaled) = shape_scaled(unscaled, shape_win);

    let (x_off, y_off) = match zoom_box {
        Some(c) => (c.x, c.y),
        _ => (0, 0),
    };

    let (x, y) = pos.into();
    let x_tf = transform(x, w_scaled, unscaled.w as f32, x_off as f32);
    let y_tf = transform(y, h_scaled, unscaled.h as f32, y_off as f32);
    ((x_tf, y_tf)).into()
}

pub trait IsSignedInt {}

impl IsSignedInt for i32 {}
impl IsSignedInt for i64 {}

#[cfg(test)]
#[macro_export]
macro_rules! point {
    ($x:literal, $y:literal) => {{
        if $x < 0.0 || $y < 0.0 {
            panic!("cannot create point from negative coords, {}, {}", $x, $y);
        }
        crate::domain::PtF { x: $x, y: $y }
    }};
}
#[cfg(test)]
#[macro_export]
macro_rules! point_i {
    ($x:literal, $y:literal) => {{
        if $x < 0 || $y < 0 {
            panic!("cannot create point from negative coords, {}, {}", $x, $y);
        }
        crate::domain::PtI { x: $x, y: $y }
    }};
}

#[macro_export]
macro_rules! impl_point_into {
    ($T:ty) => {
        impl Into<($T, $T)> for PtF {
            fn into(self) -> ($T, $T) {
                (self.x as $T, self.y as $T)
            }
        }
        impl Into<($T, $T)> for PtI {
            fn into(self) -> ($T, $T) {
                (self.x as $T, self.y as $T)
            }
        }
    };
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> From<(T, T)> for Point<T>
where
    T: Calc,
{
    fn from(value: (T, T)) -> Self {
        Self {
            x: value.0,
            y: value.1,
        }
    }
}
impl<T> Into<(T, T)> for Point<T>
where
    T: Calc,
{
    fn into(self) -> (T, T) {
        (self.x, self.y)
    }
}
impl_point_into!(i64);
impl_point_into!(i32);
pub type PtF = Point<f32>;
pub type PtI = Point<u32>;

impl PtI {
    pub fn from_signed(p: (i32, i32)) -> RvResult<Self> {
        if p.0 < 0 || p.1 < 0 {
            Err(rverr!(
                "cannot create point with negative coordinates, {:?}",
                p
            ))
        } else {
            Ok(Self {
                x: p.0 as u32,
                y: p.1 as u32,
            })
        }
    }
    pub fn equals<U>(&self, other: (U, U)) -> bool
    where
        U: PartialEq,
        PtI: Into<(U, U)>,
    {
        <Self as Into<(U, U)>>::into(*self) == other
    }
}

impl Into<PtF> for PtI {
    fn into(self) -> PtF {
        ((self.x as f32), (self.y as f32)).into()
    }
}
impl From<PtF> for PtI {
    fn from(p: PtF) -> Self {
        ((p.x as u32), (p.y as u32)).into()
    }
}
impl From<(f32, f32)> for PtI {
    fn from(x: (f32, f32)) -> Self {
        ((x.0 as u32), (x.1 as u32)).into()
    }
}
impl From<(usize, usize)> for PtI {
    fn from(x: (usize, usize)) -> Self {
        ((x.0 as u32), (x.1 as u32)).into()
    }
}

impl Into<(usize, usize)> for PtI {
    fn into(self) -> (usize, usize) {
        (self.x as usize, self.y as usize)
    }
}

fn chain_corners<T>(select: impl Fn(usize) -> T) -> impl Iterator<Item = T> {
    let iter_c1 = iter::once(select(0));
    let iter_c2 = iter::once(select(1));
    let iter_c3 = iter::once(select(2));
    let iter_c4 = iter::once(select(3));
    iter_c1.chain(iter_c2).chain(iter_c3).chain(iter_c4)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct Polygon {
    points: Vec<PtI>, // should NEVER be empty, hence private!
    enclosing_bb: BB,
    is_open: bool,
}
impl Polygon {
    pub fn enclosing_bb(&self) -> BB {
        self.enclosing_bb
    }
    pub fn points(&self) -> &Vec<PtI> {
        &self.points
    }
    /// We will need this as soon as we support polygons
    fn _from_vec(points: Vec<PtI>, is_open: bool) -> RvResult<Self> {
        let enclosing_bb = BB::from_vec(&points)?;
        Ok(Self {
            points,
            enclosing_bb,
            is_open,
        })
    }
}
impl From<BB> for Polygon {
    fn from(bb: BB) -> Self {
        let points = vec![
            (bb.x, bb.y).into(),
            (bb.x + bb.w - 1, bb.y + bb.h - 1).into(),
        ];
        Polygon {
            points,
            enclosing_bb: bb,
            is_open: false,
        }
    }
}

#[derive(Clone, Copy)]
pub enum OutOfBoundsMode {
    Deny,
    Resize(Shape), // minimal area the box needs to keep
}

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

    pub fn intersect(&self, other: BB) -> BB {
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

    pub fn max_corner_squaredist(&self, other: &BB) -> (usize, usize, i64) {
        (0..4)
            .map(|csidx| {
                let (coidx, d) = (0..4)
                    .map(|coidx| {
                        let cs = self.corner(csidx);
                        let co = other.corner(coidx);
                        let d =
                            (co.x as i64 - cs.x as i64).pow(2) + (co.y as i64 - cs.y as i64).pow(2);
                        (coidx, d)
                    })
                    .max_by_key(|(_, d)| *d)
                    .unwrap();
                (csidx, coidx, d)
            })
            .max_by_key(|(_, _, d)| *d)
            .unwrap()
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
    pub fn corners<'a>(&'a self) -> impl Iterator<Item = PtI> + 'a {
        chain_corners(|i| self.corner(i))
    }

    pub fn corner(&self, idx: usize) -> PtI {
        let (x, y, w, h) = (self.x, self.y, self.w, self.h);
        match idx {
            0 => (x, y).into(),
            1 => (x, y + h).into(),
            2 => (x + w, y + h).into(),
            3 => (x + w, y).into(),
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
            w: x_max - x_min,
            h: y_max - y_min,
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
        let x_shift: i32 = to.x as i32 - from.x as i32;
        let y_shift: i32 = to.y as i32 - from.y as i32;
        self.translate(x_shift, y_shift, shape, oob_mode)
    }

    pub fn covers_y(&self, y: u32) -> bool {
        self.y_max() > y && self.y <= y
    }
    pub fn covers_x(&self, x: u32) -> bool {
        self.x_max() > x && self.x <= x
    }

    pub fn contains<P>(&self, p: P) -> bool
    where
        P: Into<PtI>,
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
        if self.corners().any(|c| other.contains(c)) {
            true
        } else {
            other.corners().any(|c| self.contains(c))
        }
    }
}

// if any boundary line is out of view, the corresponding value is None
#[derive(Clone, Copy, Debug)]
pub struct ViewCorners {
    pub x_min: Option<u32>,
    pub y_min: Option<u32>,
    pub x_max: Option<u32>,
    pub y_max: Option<u32>,
}
impl ViewCorners {
    pub fn new(
        x_min: Option<u32>,
        y_min: Option<u32>,
        x_max: Option<u32>,
        y_max: Option<u32>,
    ) -> Self {
        Self {
            x_min,
            y_min,
            x_max,
            y_max,
        }
    }

    pub fn from_some(x_min: u32, y_min: u32, x_max: u32, y_max: u32) -> Self {
        Self::new(Some(x_min), Some(y_min), Some(x_max), Some(y_max))
    }

    pub fn to_optional_tuple(self) -> Option<(u32, u32, u32, u32)> {
        if let Self {
            x_min: Some(x_min),
            y_min: Some(y_min),
            x_max: Some(x_max),
            y_max: Some(y_max),
        } = self
        {
            Some((x_min, y_min, x_max, y_max))
        } else {
            None
        }
    }

    pub fn to_tuple_of_options(self) -> (Option<u32>, Option<u32>, Option<u32>, Option<u32>) {
        (self.x_min, self.y_min, self.x_max, self.y_max)
    }

    pub fn to_bb(self) -> Option<BB> {
        if let Some((xmin, ymin, xmax, ymax)) = self.to_optional_tuple() {
            Some(BB::from_points((xmin, ymin).into(), (xmax, ymax).into()))
        } else {
            None
        }
    }

    pub fn corner(&self, i: usize) -> Option<(u32, u32)> {
        let Self {
            x_min,
            y_min,
            x_max,
            y_max,
        } = self;
        match i {
            0 => x_min.and_then(|xmin| y_min.map(|ymin| (xmin, ymin))),
            1 => x_min.and_then(|xmin| y_max.map(|ymax| (xmin, ymax))),
            2 => x_max.and_then(|xmax| y_max.map(|ymax| (xmax, ymax))),
            3 => x_max.and_then(|xmax| y_min.map(|ymin| (xmax, ymin))),
            _ => panic!("there are only 4 corners"),
        }
    }
}

/// Iterate corners that are in view
pub struct BbViewCornerIterator {
    arriter: Flatten<core::array::IntoIter<Option<(u32, u32)>, 5>>,
}
impl BbViewCornerIterator {
    pub fn new(view_corners: ViewCorners) -> Self {
        Self {
            arriter: [
                view_corners.corner(0),
                view_corners.corner(1),
                view_corners.corner(2),
                view_corners.corner(3),
                view_corners.corner(0),
            ]
            .into_iter()
            .flatten(),
        }
    }
}
impl Iterator for BbViewCornerIterator {
    type Item = (u32, u32);
    fn next(&mut self) -> Option<Self::Item> {
        self.arriter.next()
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

pub fn zoom_box_mouse_wheel(zoom_box: Option<BB>, shape_orig: Shape, y_delta: f32) -> Option<BB> {
    let current_zb = if let Some(zb) = zoom_box {
        zb
    } else {
        BB::from_arr(&[0, 0, shape_orig.w, shape_orig.h])
    };
    let clip_val = 1.0;
    let y_delta_clipped = if y_delta > 0.0 {
        y_delta.min(clip_val)
    } else {
        y_delta.max(-clip_val)
    };
    let factor = 1.0 - y_delta_clipped * 0.1;

    Some(current_zb.center_scale(factor, shape_orig))
}

#[cfg(test)]
pub fn make_test_bbs() -> Vec<BB> {
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
fn test_polygon() {
    let bbs = make_test_bbs();
    let poly = Polygon::from(bbs[2]);
    assert_eq!(poly.enclosing_bb(), bbs[2]);
    let corners = bbs[0].corners().collect::<Vec<_>>();
    let ebb = BB::from_vec(&corners).unwrap();
    let poly = Polygon::from(ebb);
    assert_eq!(poly.enclosing_bb(), ebb);
}

#[test]
fn test_zb() {
    fn test(zb: Option<BB>, y_delta: f32, reference_coords: &[u32; 4]) {
        println!("y_delta {}", y_delta);
        let shape = Shape::new(200, 100);
        let zb_new = zoom_box_mouse_wheel(zb, shape, y_delta);
        assert_eq!(zb_new, Some(BB::from_arr(reference_coords)));
    }
    test(None, 1.0, &[10, 5, 180, 90]);
    test(None, -1.0, &[0, 0, 200, 100]);
}

#[test]
fn test_bb() {
    let bb = BB {
        x: 10,
        y: 10,
        w: 10,
        h: 10,
    };
    assert!(!bb.contains((20u32, 20u32)));
    assert!(bb.contains((10u32, 10u32)));
    assert!(bb.corner(0).equals((10, 10)));
    assert!(bb.corner(1).equals((10, 20)));
    assert!(bb.corner(2).equals((20, 20)));
    assert!(bb.corner(3).equals((20, 10)));
    assert!(bb.opposite_corner(0).equals((20, 20)));
    assert!(bb.opposite_corner(1).equals((20, 10)));
    assert!(bb.opposite_corner(2).equals((10, 10)));
    assert!(bb.opposite_corner(3).equals((10, 20)));
    for (c, i) in bb.corners().zip(0..4) {
        assert_eq!(c, bb.corner(i));
    }
    let shape = Shape::new(100, 100);
    let bb1 = bb.translate(1, 1, shape, OutOfBoundsMode::Deny);
    assert_eq!(
        bb1,
        Some(BB {
            x: 11,
            y: 11,
            w: 10,
            h: 10
        })
    );
    let shape = Shape::new(100, 100);
    let bb1 = bb.shift_max(1, 1, shape);
    assert_eq!(
        bb1,
        Some(BB {
            x: 10,
            y: 10,
            w: 11,
            h: 11
        })
    );
    let bb1 = bb.shift_max(100, 1, shape);
    assert_eq!(bb1, None);
    let bb1 = bb.shift_max(-1, -2, shape);
    assert_eq!(
        bb1,
        Some(BB {
            x: 10,
            y: 10,
            w: 9,
            h: 8
        })
    );
    let bb1 = bb.shift_max(-100, -200, shape);
    assert_eq!(bb1, None);
}

#[test]
fn test_has_overlap() {
    let bb1 = BB::from_arr(&[5, 5, 10, 10]);
    let bb2 = BB::from_arr(&[5, 5, 10, 10]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[0, 0, 10, 10]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[0, 0, 11, 11]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[2, 2, 5, 5]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[5, 5, 9, 9]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[7, 7, 12, 12]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[17, 17, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[17, 17, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[17, 3, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
    let bb2 = BB::from_arr(&[3, 17, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
}

#[test]
fn test_max_corner_dist() {
    let bb1 = BB::from_arr(&[5, 5, 10, 10]);
    let bb2 = BB::from_arr(&[5, 5, 10, 10]);
    assert_eq!(bb1.max_corner_squaredist(&bb2), (3, 1, 200));
    let bb2 = BB::from_arr(&[6, 5, 10, 10]);
    assert_eq!(bb1.max_corner_squaredist(&bb2), (1, 3, 221));
    let bb2 = BB::from_arr(&[6, 6, 10, 10]);
    assert_eq!(bb1.max_corner_squaredist(&bb2), (0, 2, 242));
    let bb2 = BB::from_arr(&[15, 15, 10, 10]);
    assert_eq!(bb1.max_corner_squaredist(&bb2), (0, 2, 800));
    let bb2 = BB::from_arr(&[15, 5, 10, 10]);
    assert_eq!(bb1.max_corner_squaredist(&bb2), (1, 3, 500));
}

#[test]
fn test_intersect() {
    let bb = BB::from_arr(&[10, 15, 20, 10]);
    assert_eq!(bb.intersect(bb), bb);
    assert_eq!(
        bb.intersect(BB::from_arr(&[5, 7, 10, 10])),
        BB::from_arr(&[10, 15, 5, 2])
    );
    assert_eq!(bb.intersect_or_self(None), bb);
    assert_eq!(
        bb.intersect_or_self(Some(BB::from_arr(&[5, 7, 10, 10]))),
        BB::from_arr(&[10, 15, 5, 2])
    );
}
