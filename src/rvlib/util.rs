use std::{
    ffi::OsStr,
    io,
    ops::{Add, Range, Sub},
    path::{Path, PathBuf},
};

use crate::format_rverr;
use crate::result::RvError;
use crate::{
    result::{to_rv, RvResult},
    types::{ResultImage, ViewImage},
};
use core::cmp::Ordering::{Greater, Less};
use image::{buffer::ConvertBuffer, DynamicImage, GenericImage, ImageBuffer, Luma, Rgb, Rgba};
use pixels::Pixels;
use std::str::FromStr;
use winit::dpi::PhysicalSize;

pub trait PixelEffect: FnMut(u32, u32) {}
impl<T: FnMut(u32, u32)> PixelEffect for T {}
pub fn filename_in_tmpdir(path: &str, tmpdir: &str) -> RvResult<String> {
    let path = PathBuf::from_str(path).unwrap();
    let fname = osstr_to_str(path.file_name()).map_err(to_rv)?;
    Path::new(tmpdir)
        .join(fname)
        .to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format_rverr!("could not transform {:?} to &str", fname))
}

pub fn path_to_str(p: &Path) -> RvResult<&str> {
    osstr_to_str(Some(p.as_os_str()))
        .map_err(|e| format_rverr!("could not transform '{:?}' due to '{:?}'", p, e))
}
pub fn osstr_to_str(p: Option<&OsStr>) -> io::Result<&str> {
    p.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{:?} not found", p)))?
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{:?} not convertible to unicode", p),
            )
        })
}

pub fn mouse_pos_transform(
    pixels: &Pixels,
    input_pos: Option<(f32, f32)>,
) -> Option<(usize, usize)> {
    pixels
        .window_pos_to_pixel(input_pos.unwrap_or((-1.0, -1.0)))
        .ok()
}

/// Converts the position of a pixel in the view to the coordinates of the original image
pub fn view_pos_to_orig_pos(
    view_pos: (u32, u32),
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> (u32, u32) {
    let unscaled = shape_unscaled(zoom_box, shape_orig);
    let scaled = shape_scaled(unscaled, shape_win);

    let (x_off, y_off) = match zoom_box {
        Some(c) => (c.x, c.y),
        _ => (0, 0),
    };

    let coord_trans_2_orig = |x: u32, n_transformed: u32, n_orig: u32| -> u32 {
        (x as f64 / n_transformed as f64 * n_orig as f64) as u32
    };
    let (x, y) = view_pos;
    let x_orig = x_off + coord_trans_2_orig(x, scaled.w, unscaled.w);
    let y_orig = y_off + coord_trans_2_orig(y, scaled.h, unscaled.h);
    (x_orig, y_orig)
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
pub fn read_image(path: &str) -> ResultImage {
    image::io::Reader::open(path)
        .map_err(to_rv)?
        .with_guessed_format()
        .map_err(to_rv)?
        .decode()
        .map_err(|e| format_rverr!("could not decode image {:?}. {:?}", path, e))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
/// shape without scaling according to zoom
pub fn shape_unscaled(zoom_box: &Option<BB>, shape_orig: Shape) -> Shape {
    zoom_box.map_or(shape_orig, |z| z.shape())
}

pub fn clipped_add<T>(x1: T, x2: T, clip_value: T) -> T
where
    T: Add<Output = T> + Sub<Output = T> + PartialOrd + Copy,
{
    if x1 >= clip_value || x2 >= clip_value || clip_value - x1 < x2 {
        clip_value
    } else {
        x1 + x2
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BB {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
impl BB {
    pub fn to_other_coords(&self, other: &BB) -> Option<Self> {
        let p_min = other.to_my_coordinates((self.x, self.y));
        p_min.map(|p| BB::from_points(p, (p.0 + self.w, p.1 + self.h)))
    }
    fn to_my_coordinates(&self, image_coords: (u32, u32)) -> Option<(u32, u32)> {
        if self.contains(image_coords) {
            Some((image_coords.0 - self.x, image_coords.1 - self.y))
        } else {
            None
        }
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
    pub fn effect_per_inner_pixel<F>(&self, mut effect: F)
    where
        F: PixelEffect,
    {
        for y in (self.y + 1)..(self.y + self.h - 1) {
            for x in (self.x + 1)..(self.x + self.w - 1) {
                effect(x, y);
            }
        }
    }
    pub fn center(&self) -> (u32, u32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
    pub fn contains(&self, p: (u32, u32)) -> bool {
        let BB { x, y, h, w } = self;
        x <= &p.0 && p.0 < x + w && y <= &p.1 && p.1 < y + h
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
    pub fn follow_movement(&self, from: (u32, u32), to: (u32, u32), shape: Shape) -> Option<Self> {
        let x_shift: i32 = to.0 as i32 - from.0 as i32;
        let y_shift: i32 = to.1 as i32 - from.1 as i32;
        let x_new = self.x as i32 + x_shift;
        let y_new = self.y as i32 + y_shift;
        if x_new >= 0
            && y_new >= 0
            && x_new as u32 + self.w < shape.w
            && y_new as u32 + self.h < shape.h
        {
            Some(Self {
                x: x_new as u32,
                y: y_new as u32,
                w: self.w,
                h: self.h,
            })
        } else {
            None
        }
    }
    pub fn extend_max(&self, amount: (u32, u32), shape: Option<Shape>) -> Self {
        let (w, h) = match shape {
            Some(shp) => (
                (self.w + amount.0).min(shp.w),
                (self.h + amount.1).min(shp.h),
            ),
            None => (self.w + amount.0, self.h + amount.1),
        };
        BB {
            x: self.x,
            y: self.y,
            w,
            h,
        }
    }
}

pub fn apply_to_matched_image<FnRgb8, FnRgba8, FnLuma8, FnRgb32F, T>(
    im_d: &DynamicImage,
    fn_rgb8: FnRgb8,
    fn_rgba8: FnRgba8,
    fn_luma8: FnLuma8,
    fn_rgb32f: FnRgb32F,
) -> T
where
    FnRgb8: Fn(&ImageBuffer<Rgb<u8>, Vec<u8>>) -> T,
    FnRgba8: Fn(&ImageBuffer<Rgba<u8>, Vec<u8>>) -> T,
    FnLuma8: Fn(&ImageBuffer<Luma<u8>, Vec<u8>>) -> T,
    FnRgb32F: Fn(&ImageBuffer<Rgb<f32>, Vec<f32>>) -> T,
{
    match im_d {
        DynamicImage::ImageRgb8(im) => fn_rgb8(im),
        DynamicImage::ImageRgba8(im) => fn_rgba8(im),
        DynamicImage::ImageLuma8(im) => fn_luma8(im),
        DynamicImage::ImageRgb32F(im) => fn_rgb32f(im),
        _ => panic!("Unsupported image type. {:?}", im_d.color()),
    }
}

pub fn orig_to_0_255(
    im_orig: &DynamicImage,
    im_mask: &Option<ImageBuffer<Luma<u8>, Vec<u8>>>,
) -> ViewImage {
    let fn_rgb32f = |im: &ImageBuffer<Rgb<f32>, Vec<f32>>| {
        let mut im = im.clone();
        let max_val = im
            .as_raw()
            .iter()
            .copied()
            .max_by(|a, b| {
                if a.is_nan() {
                    Greater
                } else if b.is_nan() {
                    Less
                } else {
                    a.partial_cmp(b).unwrap()
                }
            })
            .expect("an image should have a maximum value");
        if max_val <= 1.0 {
            for y in 0..im.height() {
                for x in 0..im.width() {
                    let p = im.get_pixel_mut(x, y);
                    p.0 = [p.0[0] * 255.0, p.0[1] * 255.0, p.0[2] * 255.0];
                }
            }
        } else if max_val > 255.0 {
            for y in 0..im.height() {
                for x in 0..im.width() {
                    let is_pixel_relevant = if let Some(im_mask) = im_mask {
                        im_mask.get_pixel(x, y)[0] > 0
                    } else {
                        true
                    };
                    let p = im.get_pixel_mut(x, y);
                    p.0 = if is_pixel_relevant {
                        [
                            p.0[0] / max_val * 255.0,
                            p.0[1] / max_val * 255.0,
                            p.0[2] / max_val * 255.0,
                        ]
                    } else {
                        [0.0, 0.0, 0.0]
                    };
                }
            }
        }
        im.convert()
    };
    apply_to_matched_image(
        im_orig,
        |im| im.clone(),
        |im| im.convert(),
        |im| im.convert(),
        fn_rgb32f,
    )
}
pub fn effect_per_pixel<F: PixelEffect>(shape: Shape, mut f: F) {
    for y in 0..shape.h {
        for x in 0..shape.w {
            f(x, y);
        }
    }
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
    assert_eq!(bb.to_my_coordinates((20, 20)), None);
    assert_eq!(bb.to_my_coordinates((10, 10)), Some((0, 0)));
    assert_eq!(bb.to_my_coordinates((11, 12)), Some((1, 2)));
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
