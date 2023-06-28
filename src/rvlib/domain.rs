use image::GenericImage;
use pixels::Pixels;
use serde::{Deserialize, Serialize};
use winit::dpi::PhysicalSize;

use std::{
    fmt::Display,
    iter::{self, Flatten},
    ops::Range,
    str::FromStr,
};

use crate::{
    result::{to_rv, RvError, RvResult},
    rverr,
};

pub fn mouse_pos_transform(
    pixels: &Pixels,
    input_pos: Option<(f32, f32)>,
) -> Option<(usize, usize)> {
    pixels
        .window_pos_to_pixel(input_pos.unwrap_or((-1.0, -1.0)))
        .ok()
}

pub fn pos_transform<F>(
    pos: (u32, u32),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
    transform: F,
) -> (u32, u32)
where
    F: Fn(u32, u32, u32, u32) -> u32,
{
    let unscaled = shape_unscaled(zoom_box, shape_orig);
    let scaled = shape_scaled(unscaled, shape_win);

    let (x_off, y_off) = match zoom_box {
        Some(c) => (c.x, c.y),
        _ => (0, 0),
    };

    let (x, y) = pos;
    let x_tf = transform(x, scaled.w, unscaled.w, x_off);
    let y_tf = transform(y, scaled.h, unscaled.h, y_off);
    (x_tf, y_tf)
}

/// Converts the position of a pixel in the view to the coordinates of the original image
pub fn view_pos_to_orig_pos(
    view_pos: (u32, u32),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> (u32, u32) {
    let coord_view_2_orig = |x: u32, n_transformed: u32, n_orig: u32, off: u32| -> u32 {
        let tmp = x as f64 * n_orig as f64 / n_transformed as f64;
        let tmp = if n_transformed > n_orig {
            tmp.ceil()
        } else {
            tmp.floor()
        };
        off + tmp as u32
    };
    pos_transform(view_pos, shape_orig, shape_win, zoom_box, coord_view_2_orig)
}
fn coord_orig_2_view(x: u32, n_transformed: u32, n_orig: u32, off: u32) -> u32 {
    let tmp = (x - off) as f64 * n_transformed as f64 / n_orig as f64;
    let tmp = if n_transformed > n_orig {
        tmp.floor()
    } else {
        tmp.ceil()
    };
    tmp as u32
}

pub fn orig_coord_to_view_coord(
    coord: u32,
    n_coords: u32,
    n_pixels_scaled: u32,
    min_max: &Option<(u32, u32)>,
) -> Option<u32> {
    if let Some((min, max)) = min_max {
        if &coord < min || max <= &coord {
            return None;
        }
    }
    let unscaled = min_max.map_or(n_coords, |mm| mm.1 - mm.0);
    let off = min_max.map_or(0, |mm| mm.0);
    Some(coord_orig_2_view(coord, n_pixels_scaled, unscaled, off))
}
/// Converts the position of a pixel in the view to the coordinates of the original image
pub fn orig_pos_to_view_pos(
    orig_pos: (u32, u32),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> Option<(u32, u32)> {
    if let Some(zb) = zoom_box {
        if !zb.contains(orig_pos) {
            return None;
        }
    }
    Some(pos_transform(
        orig_pos,
        shape_orig,
        shape_win,
        zoom_box,
        coord_orig_2_view,
    ))
}

pub fn mouse_pos_to_orig_pos(
    mouse_pos: Option<(usize, usize)>,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> Option<(u32, u32)> {
    mouse_pos
        .map(|(x, y)| view_pos_to_orig_pos((x as u32, y as u32), shape_orig, shape_win, zoom_box))
        .filter(|(x_orig, y_orig)| x_orig < &shape_orig.w && y_orig < &shape_orig.h)
}

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
    pub fn from_size(size: &PhysicalSize<u32>) -> Self {
        Self {
            w: size.width,
            h: size.height,
        }
    }
}
/// shape of the image that fits into the window
pub fn shape_scaled(shape_unscaled: Shape, shape_win: Shape) -> Shape {
    let w_ratio = shape_unscaled.w as f64 / shape_win.w as f64;
    let h_ratio = shape_unscaled.h as f64 / shape_win.h as f64;
    let ratio = w_ratio.max(h_ratio);
    let w_new = (shape_unscaled.w as f64 / ratio) as u32;
    let h_new = (shape_unscaled.h as f64 / ratio) as u32;
    Shape { w: w_new, h: h_new }
}
/// shape without scaling to window
pub fn shape_unscaled(zoom_box: &Option<BB>, shape_orig: Shape) -> Shape {
    zoom_box.map_or(shape_orig, |z| z.shape())
}

pub type CornerOptions = ((Option<u32>, Option<u32>), (Option<u32>, Option<u32>));

pub type Point = (u32, u32);

#[cfg(test)]
fn find_enclosing_bb(points: &Vec<(u32, u32)>) -> RvResult<BB> {
    if points.is_empty() {
        Err(rverr!("need points to compute enclosing bounding box",))
    } else {
        fn pick_better(v: u32, new_v: u32, cmp: fn(u32, u32) -> bool) -> u32 {
            if cmp(new_v, v) {
                new_v
            } else {
                v
            }
        }

        let smaller = |a, b| a < b;
        let greater = |a, b| a > b;

        let (mut x_min, mut y_min, mut x_max, mut y_max) = (u32::MAX, u32::MAX, u32::MIN, u32::MIN);
        for p in points {
            x_min = pick_better(x_min, p.0, smaller);
            y_min = pick_better(y_min, p.1, smaller);
            x_max = pick_better(x_max, p.0, greater);
            y_max = pick_better(y_max, p.1, greater);
        }
        Ok(BB::from_points((x_min, y_min), (x_max, y_max)))
    }
}

fn chain_corners<T>(select: impl Fn(usize) -> T) -> impl Iterator<Item = T> {
    let iter_c1 = iter::once(select(0));
    let iter_c2 = iter::once(select(1));
    let iter_c3 = iter::once(select(2));
    let iter_c4 = iter::once(select(3));
    iter_c1.chain(iter_c2).chain(iter_c3).chain(iter_c4)
}

pub trait MakeDrawable {
    type BoundaryPointIterator;
    type PointIterator;
    fn points_on_view(
        &self,
        shape_view: Shape,
        shape_orig: Shape,
        shape_win: Shape,
        zoom_box: &Option<BB>,
    ) -> (Self::BoundaryPointIterator, Self::PointIterator);
    fn enclosing_bb(&self) -> BB;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct Polygon {
    points: Vec<Point>, // should NEVER be empty, hence private!
    enclosing_bb: BB,
}
impl Polygon {
    pub fn from_bb(bb: BB) -> Self {
        let points = vec![(bb.x, bb.y), (bb.x + bb.w - 1, bb.y + bb.h - 1)];
        Polygon {
            points,
            enclosing_bb: bb,
        }
    }
    pub fn enclosing_bb(&self) -> BB {
        self.enclosing_bb
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
            (self.x.max(other.x), self.y.max(other.y)),
            (
                self.x_max().min(other.x_max()),
                self.y_max().min(other.y_max()),
            ),
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
                            (co.0 as i64 - cs.0 as i64).pow(2) + (co.1 as i64 - cs.1 as i64).pow(2);
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
    pub fn corners<'a>(&'a self) -> impl Iterator<Item = (u32, u32)> + 'a {
        chain_corners(|i| self.corner(i))
    }

    pub fn corner(&self, idx: usize) -> (u32, u32) {
        let (x, y, w, h) = (self.x, self.y, self.w, self.h);
        match idx {
            0 => (x, y),
            1 => (x, y + h),
            2 => (x + w, y + h),
            3 => (x + w, y),
            _ => panic!("bounding boxes only have 4, {idx} is out of bounds"),
        }
    }
    pub fn opposite_corner(&self, idx: usize) -> (u32, u32) {
        self.corner((idx + 2) % 4)
    }

    pub fn shape(&self) -> Shape {
        Shape {
            w: self.w,
            h: self.h,
        }
    }

    pub fn from_points(p1: (u32, u32), p2: (u32, u32)) -> Self {
        let x_min = p1.0.min(p2.0);
        let y_min = p1.1.min(p2.1);
        let x_max = p1.0.max(p2.0);
        let y_max = p1.1.max(p2.1);
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

    pub fn center(&self) -> (u32, u32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }

    pub fn min_usize(&self) -> (usize, usize) {
        (self.x as usize, self.y as usize)
    }

    pub fn max_usize(&self) -> (usize, usize) {
        ((self.x + self.w) as usize, (self.y + self.h) as usize)
    }

    pub fn min(&self) -> (u32, u32) {
        (self.x, self.y)
    }

    pub fn max(&self) -> (u32, u32) {
        (self.x + self.w, self.y + self.h)
    }

    pub fn follow_movement(
        &self,
        from: (u32, u32),
        to: (u32, u32),
        shape: Shape,
        oob_mode: OutOfBoundsMode,
    ) -> Option<Self> {
        let x_shift: i32 = to.0 as i32 - from.0 as i32;
        let y_shift: i32 = to.1 as i32 - from.1 as i32;
        self.translate(x_shift, y_shift, shape, oob_mode)
    }

    pub fn covers_y(&self, y: u32) -> bool {
        self.y_max() > y && self.y <= y
    }
    pub fn covers_x(&self, x: u32) -> bool {
        self.x_max() > x && self.x <= x
    }

    pub fn contains(&self, p: (u32, u32)) -> bool {
        self.covers_x(p.0) && self.covers_y(p.1)
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

    pub fn to_viewcorners(
        &self,
        shape_orig: Shape,
        shape_win: Shape,
        zoom_box: &Option<BB>,
    ) -> ViewCorners {
        let (x_min, y_min, x_max, y_max) = match zoom_box {
            Some(zb) => {
                let x_min = if zb.x > self.x { None } else { Some(self.x) };
                let y_min = if zb.y > self.y { None } else { Some(self.y) };
                let x_max = if zb.x_max() < self.x_max() {
                    None
                } else {
                    Some(self.x_max())
                };
                let y_max = if zb.y_max() < self.y_max() {
                    None
                } else {
                    Some(self.y_max())
                };

                (x_min, y_min, x_max, y_max)
            }
            None => ViewCorners::from_some(self.x, self.y, self.x_max(), self.y_max())
                .to_tuple_of_options(),
        };
        let s_unscaled = shape_unscaled(zoom_box, shape_orig);
        let s_scaled = shape_scaled(s_unscaled, shape_win);
        let tf_x = |x: Option<u32>| {
            x.and_then(|x| {
                orig_coord_to_view_coord(
                    x,
                    s_unscaled.w,
                    s_scaled.w,
                    &zoom_box.map(|zb| zb.min_max(0)),
                )
            })
        };
        let tf_y = |y: Option<u32>| {
            y.and_then(|y| {
                orig_coord_to_view_coord(
                    y,
                    s_unscaled.h,
                    s_scaled.h,
                    &zoom_box.map(|zb| zb.min_max(1)),
                )
            })
        };
        ViewCorners::new(tf_x(x_min), tf_y(y_min), tf_x(x_max), tf_y(y_max))
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
            Some(BB::from_points((xmin, ymin), (xmax, ymax)))
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

pub struct BbPointIterator {
    bb: BB,
    x: u32,
    y: u32,
}

impl BbPointIterator {
    pub fn new(view_corners: ViewCorners, view_shape: Shape) -> Self {
        let ViewCorners {
            x_min,
            y_min,
            x_max,
            y_max,
        } = view_corners;
        let x_min = x_min.unwrap_or(0);
        let y_min = y_min.unwrap_or(0);
        let x_max = x_max.unwrap_or(view_shape.w);
        let y_max = y_max.unwrap_or(view_shape.h);
        let bb = BB::from_arr(&[x_min, y_min, x_max - x_min, y_max - y_min]);
        Self {
            bb,
            x: bb.x,
            y: bb.y,
        }
    }
    pub fn from_bb(bb: BB) -> Self {
        BbPointIterator {
            bb,
            x: bb.x,
            y: bb.y,
        }
    }
}
impl Iterator for BbPointIterator {
    type Item = (u32, u32);
    fn next(&mut self) -> Option<Self::Item> {
        let (x, y) = (self.x, self.y);
        let (x_max_excl, y_max_excl) = self.bb.max();
        // we need to check also for x since we might have a degenerated box with width 0
        if self.y == y_max_excl || self.x == x_max_excl {
            None
        } else {
            (self.x, self.y) = if self.x == x_max_excl - 1 {
                (self.bb.x, self.y + 1)
            } else {
                (self.x + 1, self.y)
            };
            Some((x, y))
        }
    }
}

impl MakeDrawable for BB {
    type BoundaryPointIterator = BbViewCornerIterator;
    type PointIterator = BbPointIterator;
    fn points_on_view(
        &self,
        shape_view: Shape,
        shape_orig: Shape,
        shape_win: Shape,
        zoom_box: &Option<BB>,
    ) -> (Self::BoundaryPointIterator, Self::PointIterator) {
        let view_corners = self.to_viewcorners(shape_orig, shape_win, zoom_box);
        let boundary = BbViewCornerIterator::new(view_corners);
        let inner = BbPointIterator::new(view_corners, shape_view);
        (boundary, inner)
    }
    fn enclosing_bb(&self) -> BB {
        *self
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
    let poly = Polygon::from_bb(bbs[2]);
    assert_eq!(poly.enclosing_bb(), bbs[2]);
    let corners = bbs[0].corners().collect();
    let ebb = find_enclosing_bb(&corners).unwrap();
    let poly = Polygon::from_bb(ebb);
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
    assert!(!bb.contains((20, 20)));
    assert!(bb.contains((10, 10)));
    assert_eq!(bb.corner(0), (10, 10));
    assert_eq!(bb.corner(1), (10, 20));
    assert_eq!(bb.corner(2), (20, 20));
    assert_eq!(bb.corner(3), (20, 10));
    assert_eq!(bb.opposite_corner(0), (20, 20));
    assert_eq!(bb.opposite_corner(1), (20, 10));
    assert_eq!(bb.opposite_corner(2), (10, 10));
    assert_eq!(bb.opposite_corner(3), (10, 20));
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
fn test_to_orig_pos() {
    let orig_pos = mouse_pos_to_orig_pos(
        Some((0, 0)),
        Shape { w: 120, h: 120 },
        Shape { w: 120, h: 120 },
        &None,
    );
    assert_eq!(Some((0, 0)), orig_pos);
    let orig_pos = mouse_pos_to_orig_pos(
        Some((0, 0)),
        Shape { w: 120, h: 120 },
        Shape { w: 20, h: 20 },
        &Some(BB {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        }),
    );
    assert_eq!(Some((10, 10)), orig_pos);
    let orig_pos = mouse_pos_to_orig_pos(
        Some((19, 19)),
        Shape { w: 120, h: 120 },
        Shape { w: 20, h: 20 },
        &Some(BB {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        }),
    );
    assert_eq!(Some((29, 29)), orig_pos);
    let orig_pos = mouse_pos_to_orig_pos(
        Some((10, 10)),
        Shape { w: 120, h: 120 },
        Shape { w: 20, h: 20 },
        &Some(BB {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        }),
    );
    assert_eq!(Some((20, 20)), orig_pos);
}
#[test]
fn test_view_pos_tf() {
    let shape_orig = Shape { w: 20, h: 10 };
    let shape_win = Shape { w: 40, h: 20 };
    assert_eq!(
        orig_pos_to_view_pos((4, 4), shape_orig, shape_win, &None),
        Some((8, 8))
    );
    fn test_inverse(shape_orig: Shape, shape_win: Shape, zoom_box: &Option<BB>, tol: i32) {
        let view_pos = (10, 10);
        let orig_pos = view_pos_to_orig_pos((10, 10), shape_orig, shape_win, zoom_box);
        let view_pos_ = orig_pos_to_view_pos(orig_pos, shape_orig, shape_win, zoom_box);
        println!("view pos_ {:?}", view_pos_);
        assert!((view_pos.0 as i32 - view_pos_.unwrap().0 as i32).abs() <= tol);
        assert!((view_pos.1 as i32 - view_pos_.unwrap().1 as i32).abs() <= tol);
    }
    let shape_orig = Shape { w: 90, h: 120 };
    let shape_win = Shape { w: 320, h: 440 };
    test_inverse(shape_orig, shape_win, &None, 0);
    let shape_orig = Shape { w: 190, h: 620 };
    let shape_win = Shape { w: 120, h: 240 };
    test_inverse(shape_orig, shape_win, &None, 0);
    let shape_orig = Shape { w: 293, h: 321 };
    let shape_win = Shape { w: 520, h: 241 };
    test_inverse(shape_orig, shape_win, &None, 0);
    let shape_orig = Shape { w: 40, h: 40 };
    let shape_win = Shape { w: 40, h: 40 };
    test_inverse(
        shape_orig,
        shape_win,
        &Some(BB {
            x: 10,
            y: 10,
            w: 20,
            h: 10,
        }),
        0,
    );
    let shape_orig = Shape { w: 1040, h: 2113 };
    let shape_win = Shape { w: 401, h: 139 };
    test_inverse(
        shape_orig,
        shape_win,
        &Some(BB {
            x: 17,
            y: 10,
            w: 22,
            h: 11,
        }),
        2,
    );
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
fn test_view_corners() {
    let bb = BB::from_arr(&[5, 5, 10, 10]);
    let shape = Shape::new(40, 80);
    let view_corners = bb.to_viewcorners(shape, shape, &None);
    let bb_vc = view_corners.to_bb();
    assert_eq!(Some(bb), bb_vc);
}

#[test]
fn test_point_iterators() {
    fn test(zb: Option<BB>, bb: BB, ref_bb: BB) {
        let shape = Shape::new(2100, 100);
        let (boundary, inners) = bb.points_on_view(shape, shape, shape, &zb);
        assert_eq!(
            ref_bb
                .corners()
                .chain(iter::once(ref_bb.corner(0)))
                .collect::<Vec<_>>(),
            boundary.collect::<Vec<_>>()
        );
        let ips = inners.collect::<Vec<_>>();

        for y in ref_bb.y_range() {
            for x in ref_bb.x_range() {
                assert!(ips.contains(&(x, y)));
            }
        }

        for ip in ips {
            assert!(ip.0 >= ref_bb.x);
            assert!(ip.0 < ref_bb.x_max());
            assert!(ip.1 >= ref_bb.y);
            assert!(ip.1 < ref_bb.y_max());
        }
    }
    let bb = BB::from_arr(&[5, 5, 10, 10]);
    test(None, bb, bb);
    test(Some(BB::from_arr(&[0, 0, 100, 100])), bb, bb);
    test(
        Some(BB::from_arr(&[5, 5, 80, 80])),
        bb,
        BB::from_arr(&[0, 0, 12, 12]),
    );
    let bb_degenerated_y = BB::from_arr(&[10, 10, 5, 0]);
    test(None, bb_degenerated_y, bb_degenerated_y);
    let bb_degenerated_x = BB::from_arr(&[10, 10, 0, 5]);
    test(None, bb_degenerated_x, bb_degenerated_x);
    let bb_degenerated = BB::from_arr(&[10, 10, 0, 0]);
    test(None, bb_degenerated, bb_degenerated);
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
