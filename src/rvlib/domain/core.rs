use image::GenericImage;
use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};

use crate::{result::RvResult, rverr};

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

#[derive(Clone, Copy)]
pub enum OutOfBoundsMode {
    Deny,
    Resize(Shape), // minimal area the box needs to keep
}

pub fn max_squaredist<'a, I1, I2>(points1: I1, points2: I2) -> (PtI, PtI, i64)
where
    I1: Iterator<Item = PtI> + 'a + Clone,
    I2: Iterator<Item = PtI> + 'a + Clone,
{
    points1
        .map(|p1| {
            points2
                .clone()
                .map(|p2| {
                    let d = (p2.x as i64 - p1.x as i64).pow(2) + (p2.y as i64 - p1.y as i64).pow(2);
                    (p1, p2, d)
                })
                .max_by_key(|(_, _, d)| *d)
                .unwrap()
        })
        .max_by_key(|(_, _, d)| *d)
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
                    x: x as f32,
                    y: y as f32,
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T>
where
    T: Calc + Copy,
{
    pub fn len_square(&self) -> T {
        self.x * self.x + self.y * self.y
    }
    pub fn dist_square(&self, other: &Self) -> T {
        <(T, T) as Into<Point<T>>>::into((self.x - other.x, self.y - other.y)).len_square()
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
impl_point_into!(i64);
impl_point_into!(i32);
pub type PtF = Point<f32>;
pub type PtI = Point<u32>;

impl Mul<f32> for PtF {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

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
        ((p.x as f32), (p.y as f32)).into()
    }
}
impl From<PtF> for PtI {
    fn from(p: PtF) -> Self {
        ((p.x as u32), (p.y as u32)).into()
    }
}
impl From<(u32, u32)> for PtF {
    fn from(x: (u32, u32)) -> Self {
        ((x.0 as f32), (x.1 as f32)).into()
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
