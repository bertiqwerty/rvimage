use crate::{result::RvResult, rverr};
use image::{GenericImage, Pixel};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    ops::{Add, Div, Mul, Neg, Sub},
};

pub trait Abs {
    fn abs(self) -> Self;
}
pub trait Min {
    fn min(self, other: Self) -> Self;
}
pub trait Max {
    fn max(self, other: Self) -> Self;
}

impl<T> Min for T
where
    T: Calc,
{
    fn min(self, other: Self) -> Self {
        min(self, other)
    }
}
impl<T> Max for T
where
    T: Calc,
{
    fn max(self, other: Self) -> Self {
        max(self, other)
    }
}

macro_rules! impl_trait {
    ($trait_name:ident, $method:ident, $($T:ty),+) => {
        $(impl $trait_name for $T {
            fn $method(self) -> Self {
                self.$method()
            }
        })+
    };
}
impl Abs for TPtI {
    fn abs(self) -> Self {
        self
    }
}
impl_trait!(Abs, abs, f32, f64, i32, i64);

pub trait CoordinateBox {
    fn size_addon() -> Self;
    fn is_close_to(&self, other: Self) -> bool;
}

impl CoordinateBox for TPtI {
    fn size_addon() -> Self {
        TPtI::one()
    }
    fn is_close_to(&self, other: Self) -> bool {
        *self == other
    }
}
impl CoordinateBox for TPtF {
    fn size_addon() -> Self {
        TPtF::zero()
    }
    fn is_close_to(&self, other: Self) -> bool {
        floats_close(*self, other)
    }
}

pub trait Calc:
    Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Sized
    + PartialOrd
    + Abs
    + From<u32>
    + Into<TPtF>
    + Clone
    + Copy
{
    fn one() -> Self {
        Self::from(1)
    }
    fn zero() -> Self {
        Self::from(0)
    }
}
impl<T> Calc for T where
    T: Add<Output = Self>
        + Sub<Output = Self>
        + Mul<Output = Self>
        + Div<Output = Self>
        + Sized
        + PartialOrd
        + Abs
        + From<u32>
        + Into<TPtF>
        + Clone
        + Copy
{
}

fn floats_close(x: TPtF, y: TPtF) -> bool {
    (x - y).abs() < 1e-10
}

pub fn min_from_partial<T>(x1: &T, x2: &T) -> Ordering
where
    T: PartialOrd,
{
    match x1.partial_cmp(x2) {
        Some(o) => o,
        None => Ordering::Less,
    }
}
pub fn max_from_partial<T>(x1: &T, x2: &T) -> Ordering
where
    T: PartialOrd,
{
    match x1.partial_cmp(x2) {
        Some(o) => o,
        None => Ordering::Greater,
    }
}

pub fn min<T>(x1: T, x2: T) -> T
where
    T: PartialOrd,
{
    match min_from_partial(&x1, &x2) {
        Ordering::Greater => x2,
        _ => x1,
    }
}
pub fn max<T>(x1: T, x2: T) -> T
where
    T: PartialOrd,
{
    match max_from_partial(&x1, &x2) {
        Ordering::Less => x2,
        _ => x1,
    }
}

pub trait InstanceAnnotate: Clone + Default + PartialEq {
    fn is_contained_in_image(&self, shape: ShapeI) -> bool;
    fn contains<P>(&self, point: P) -> bool
    where
        P: Into<PtF>;
    fn dist_to_boundary(&self, p: PtF) -> TPtF;
}

pub type ShapeI = Shape<u32>;
pub type ShapeF = Shape<f64>;

impl From<ShapeI> for ShapeF {
    fn from(value: ShapeI) -> Self {
        Self {
            w: value.w as f64,
            h: value.h as f64,
        }
    }
}
impl From<ShapeF> for ShapeI {
    fn from(value: ShapeF) -> Self {
        Self {
            w: value.w as u32,
            h: value.h as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape<T>
where
    T: Calc,
{
    pub w: T,
    pub h: T,
}
impl<T> Shape<T>
where
    T: Calc,
{
    pub fn new(w: T, h: T) -> Self {
        Self { w, h }
    }
    pub fn rot90_with_image_ntimes(&self, n: u8) -> Self {
        if n % 2 == 0 {
            *self
        } else {
            Self {
                w: self.h,
                h: self.w,
            }
        }
    }
}

impl ShapeI {
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

impl From<[usize; 2]> for ShapeI {
    fn from(value: [usize; 2]) -> Self {
        Self::new(value[0] as u32, value[1] as u32)
    }
}

#[derive(Clone, Copy)]
pub enum OutOfBoundsMode<T>
where
    T: Calc,
{
    Deny,
    Resize(Shape<T>), // minimal area the box needs to keep
}

pub fn dist_lineseg_point(ls: &(PtF, PtF), p: PtF) -> f64 {
    let (p1, p2) = ls;
    let p1 = *p1;
    let p2 = *p2;
    let d = (p1 - p2).len_square().sqrt();
    let n = (p1 - p2) / d;
    let proj = p1 + n * (p - p1).dot(&n);
    if proj.x >= p1.x.min(p2.x)
        && proj.x <= p1.x.max(p2.x)
        && proj.y >= p1.y.min(p2.y)
        && proj.y <= p1.y.max(p2.y)
    {
        (p - proj).len_square().sqrt()
    } else {
        (p - p1).len_square().min((p - p2).len_square()).sqrt()
    }
}
pub fn max_squaredist<'a, T, I1, I2>(points1: I1, points2: I2) -> (Point<T>, Point<T>, T)
where
    T: Calc,
    I1: Iterator<Item = Point<T>> + 'a + Clone,
    I2: Iterator<Item = Point<T>> + 'a + Clone,
{
    points1
        .map(|p1| {
            points2
                .clone()
                .map(|p2| {
                    let dist_x = unsigned_dist(p2.x, p1.x);
                    let dist_y = unsigned_dist(p2.y, p1.y);
                    let d = dist_x * dist_x + dist_y * dist_y;
                    (p1, p2, d)
                })
                .max_by(|(_, _, d1), (_, _, d2)| max_from_partial(d1, d2))
                .unwrap()
        })
        .max_by(|(_, _, d1), (_, _, d2)| max_from_partial(d1, d2))
        .unwrap()
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

#[macro_export]
macro_rules! impl_point_into {
    ($T:ty) => {
        impl From<PtI> for ($T, $T) {
            fn from(p: PtI) -> Self {
                (p.x as $T, p.y as $T)
            }
        }
        impl From<PtF> for ($T, $T) {
            fn from(p: PtF) -> Self {
                (p.x as $T, p.y as $T)
            }
        }
        impl From<($T, $T)> for PtF {
            fn from((x, y): ($T, $T)) -> Self {
                Self {
                    x: x as f64,
                    y: y as f64,
                }
            }
        }
        impl From<($T, $T)> for PtI {
            fn from((x, y): ($T, $T)) -> Self {
                Self {
                    x: x as u32,
                    y: y as u32,
                }
            }
        }
    };
}

fn unsigned_dist<T>(x1: T, x2: T) -> T
where
    T: Sub<Output = T> + PartialOrd,
{
    if x1 > x2 {
        x1 - x2
    } else {
        x2 - x1
    }
}

pub fn clamp_sub_zero<T>(x1: T, x2: T) -> T
where
    T: Calc,
{
    if x1 < x2 {
        T::zero()
    } else {
        x1 - x2
    }
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T>
where
    T: Calc,
{
    pub fn len_square(&self) -> T {
        self.x * self.x + self.y * self.y
    }
    pub fn dist_square(&self, other: &Self) -> T
    where
        T: PartialOrd,
    {
        <(T, T) as Into<Point<T>>>::into((
            // make this work also for unsigned types
            unsigned_dist(self.x, other.x),
            unsigned_dist(self.y, other.y),
        ))
        .len_square()
    }
    pub fn dot(&self, rhs: &Self) -> T {
        self.x * rhs.x + self.y * rhs.y
    }

    /// the zero is in the bottom left of the image, y points upwards
    fn to_math_coord_sys(self, h: u32) -> Self {
        Self {
            x: self.x,
            y: T::from(h) - self.y,
        }
    }

    /// origin is the origin of the image coordinate system relative to the math coord system.
    fn to_image_coord_sys(self, origin: Point<T>) -> Self
    where
        T: Neg<Output = T>,
    {
        Self {
            x: self.x - origin.x,
            y: -(self.y - origin.y),
        }
    }

    fn rot90_in_math(&self) -> Self
    where
        T: Neg<Output = T>,
    {
        Self {
            x: -self.y,
            y: self.x,
        }
    }

    /// Mathematically positively oriented, counter clockwise, like Rot90 tool, different from image crate
    pub fn rot90_with_image(&self, shape: &ShapeI) -> Self
    where
        T: Neg<Output = T>,
    {
        let p_in_math = self.to_math_coord_sys(shape.h);
        let p_rotated = p_in_math.rot90_in_math();
        let top_right_corner_in_math = Self {
            x: T::from(shape.w),
            y: T::from(shape.h),
        };
        let origin = top_right_corner_in_math.rot90_in_math();
        p_rotated.to_image_coord_sys(origin)
    }
    pub fn rot90_with_image_ntimes(&self, shape: &ShapeI, n: u8) -> Self
    where
        T: Neg<Output = T>,
    {
        if n > 0 {
            let mut p = self.rot90_with_image(shape);
            for i in 1..n {
                let shape = shape.rot90_with_image_ntimes(i);
                p = p.rot90_with_image(&shape);
            }
            p
        } else {
            *self
        }
    }

    pub fn is_close_to(&self, other: Self) -> bool
    where
        T: CoordinateBox,
    {
        self.x.is_close_to(other.x) && self.y.is_close_to(other.y)
    }
}

impl<T> Mul<T> for Point<T>
where
    T: Calc,
{
    type Output = Self;
    fn mul(self, rhs: T) -> Self::Output {
        Point {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}
impl<T> Mul for Point<T>
where
    T: Calc,
{
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}
impl<T> Div<T> for Point<T>
where
    T: Calc,
{
    type Output = Self;
    fn div(self, rhs: T) -> Self::Output {
        Point {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}
impl<T> Div for Point<T>
where
    T: Calc,
{
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

impl<T> Sub for Point<T>
where
    T: Calc,
{
    type Output = Point<T>;
    fn sub(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}
impl<T> Add for Point<T>
where
    T: Calc,
{
    type Output = Point<T>;
    fn add(self, rhs: Self) -> Self::Output {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
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
impl<T> From<Point<T>> for (T, T)
where
    T: Calc,
{
    fn from(p: Point<T>) -> (T, T) {
        (p.x, p.y)
    }
}
impl_point_into!(i32);
pub type TPtF = f64;
pub type TPtI = u32;
pub type PtF = Point<TPtF>;
pub type PtI = Point<TPtI>;

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

impl From<PtI> for PtF {
    fn from(p: PtI) -> Self {
        ((p.x as f64), (p.y as f64)).into()
    }
}
impl From<PtF> for PtI {
    fn from(p: PtF) -> Self {
        ((p.x as u32), (p.y as u32)).into()
    }
}
impl From<(u32, u32)> for PtF {
    fn from(x: (u32, u32)) -> Self {
        ((x.0 as f64), (x.1 as f64)).into()
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

impl From<PtI> for (usize, usize) {
    fn from(p: PtI) -> Self {
        (p.x as usize, p.y as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    pub center: PtF,
    pub radius: TPtF,
}

pub fn color_with_intensity<CLR>(mut color: CLR, intensity: f64) -> CLR
where
    CLR: Pixel<Subpixel = u8>,
{
    let channels = color.channels_mut();
    for channel in channels {
        *channel = (*channel as f64 * intensity) as u8;
    }
    color
}
#[test]
fn test_rot() {
    let shape = &Shape::new(5, 3);
    let p = PtF { x: 2.0, y: 1.0 };
    let p_rot_1 = p.rot90_with_image(shape);
    assert!(p_rot_1.is_close_to(PtF { x: 1.0, y: 3.0 }));
    assert!(p
        .rot90_with_image_ntimes(shape, 2)
        .is_close_to(p_rot_1.rot90_with_image(&shape.rot90_with_image_ntimes(1))));
}
