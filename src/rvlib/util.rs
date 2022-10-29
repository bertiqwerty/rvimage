use std::{
    ffi::OsStr,
    io,
    iter::once,
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
use winit::{dpi::PhysicalSize, event::VirtualKeyCode};

pub fn with_control(key: VirtualKeyCode, f_event: impl Fn(VirtualKeyCode) -> bool) -> bool {
    f_event(key) && (f_event(VirtualKeyCode::LControl) || f_event(VirtualKeyCode::RControl))
}

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
/// shape without scaling to window
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
pub type CornerOptions = ((Option<u32>, Option<u32>), (Option<u32>, Option<u32>));
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BB {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
impl BB {
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
    pub fn corners(&self) -> impl Iterator<Item = (u32, u32)> {
        let iter_c1 = once(self.corner(0));
        let iter_c2 = once(self.corner(1));
        let iter_c3 = once(self.corner(2));
        let iter_c4 = once(self.corner(3));
        iter_c1.chain(iter_c2).chain(iter_c3).chain(iter_c4)
    }
    pub fn corner(&self, idx: usize) -> (u32, u32) {
        let (x, y, w, h) = (self.x, self.y, self.w, self.h);
        match idx {
            0 => (x, y),
            1 => (x, y + h - 1),
            2 => (x + w - 1, y + h - 1),
            3 => (x + w - 1, y),
            _ => panic!("bounding boxes only have 4, {} is out of bounds", idx),
        }
    }
    pub fn opposite_corner(&self, idx: usize) -> (u32, u32) {
        self.corner((idx + 2) % 4)
    }

    pub fn to_view_corners(
        &self,
        shape_orig: Shape,
        shape_win: Shape,
        zoom_box: &Option<BB>,
    ) -> CornerOptions {
        let ((x_min, y_min), (x_max, y_max)) = match zoom_box {
            Some(zb) => {
                let x_min = if zb.x > self.min().0 {
                    None
                } else {
                    Some(self.min().0)
                };
                let y_min = if zb.y > self.min().1 {
                    None
                } else {
                    Some(self.min().1)
                };
                let x_max = if zb.x > self.max().0 {
                    None
                } else {
                    Some(self.max().0)
                };
                let y_max = if zb.y > self.max().1 {
                    None
                } else {
                    Some(self.max().1)
                };

                ((x_min, y_min), (x_max, y_max))
            }
            None => (
                (Some(self.min().0), Some(self.min().1)),
                (Some(self.max().0), Some(self.max().1)),
            ),
        };
        let s_unscaled = shape_unscaled(zoom_box, shape_orig);
        let s_scaled = shape_scaled(s_unscaled, shape_win);
        let tf_x = |x| {
            orig_coord_to_view_coord(
                x,
                s_unscaled.w,
                s_scaled.w,
                &zoom_box.map(|zb| zb.min_max(0)),
            )
        };
        let tf_y = |y| {
            orig_coord_to_view_coord(
                y,
                s_unscaled.h,
                s_scaled.h,
                &zoom_box.map(|zb| zb.min_max(1)),
            )
        };
        (
            (x_min.and_then(tf_x), y_min.and_then(tf_y)),
            (x_max.and_then(tf_x), y_max.and_then(tf_y)),
        )
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
        self.translate(x_shift, y_shift, shape)
    }
    pub fn is_contained_in(&self, shape: Shape) -> bool {
        self.x + self.w < shape.w && self.y + self.h < shape.h
    }
    pub fn translate(&self, x_shift: i32, y_shift: i32, shape: Shape) -> Option<Self> {
        let x = (self.x as i32 + x_shift) as u32;
        let y = (self.y as i32 + y_shift) as u32;

        let bb = Self {
            x,
            y,
            w: self.w,
            h: self.h,
        };
        if bb.is_contained_in(shape) {
            Some(bb)
        } else {
            None
        }
    }
    pub fn extend_max(&self, x_shift: i32, y_shift: i32, shape: Shape) -> Option<Self> {
        let (w, h) = (self.w as i32 + x_shift, self.h as i32 + y_shift);
        let bb = BB {
            x: self.x,
            y: self.y,
            w: w as u32,
            h: h as u32,
        };
        if bb.is_contained_in(shape) {
            Some(bb)
        } else {
            None
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

pub fn to_i64(x: (u32, u32)) -> (i64, i64) {
    ((x.0 as i64), (x.1 as i64))
}
pub fn to_u32(x: (usize, usize)) -> (u32, u32) {
    ((x.0 as u32), (x.1 as u32))
}

pub fn to_01(x: u8) -> f32 {
    x as f32 / 255.0
}

pub fn apply_alpha(pixel_rgb: &[u8; 3], color: &[u8; 3], alpha: u8) -> Rgb<u8> {
    let alpha_amount = to_01(alpha);
    let apply_alpha_scalar = |x_anno, x_res| {
        ((to_01(x_anno) * alpha_amount + (1.0 - alpha_amount) * to_01(x_res)) * 255.0) as u8
    };
    let [r_pixel, g_pixel, b_pixel] = pixel_rgb;
    let [r_clr, g_clr, b_clr] = color;
    Rgb([
        apply_alpha_scalar(*r_pixel, *r_clr),
        apply_alpha_scalar(*g_pixel, *g_clr),
        apply_alpha_scalar(*b_pixel, *b_clr),
    ])
}

pub fn draw_bx_on_image<I: GenericImage, F: Fn(&I::Pixel) -> I::Pixel>(
    mut im: I,
    corner_1: (Option<u32>, Option<u32>),
    corner_2: (Option<u32>, Option<u32>),
    color: &I::Pixel,
    fn_inner_color: F,
) -> I {
    let x_c1 = corner_1.0.unwrap_or(0);
    let y_c1 = corner_1.1.unwrap_or(0);
    let x_c2 = corner_2.0.unwrap_or_else(|| im.width());
    let y_c2 = corner_2.1.unwrap_or_else(|| im.height());
    let x_min = x_c1.min(x_c2);
    let y_min = y_c1.min(y_c2);
    let x_max = x_c1.max(x_c2);
    let y_max = y_c1.max(y_c2);
    let draw_bx = BB {
        x: x_min as u32,
        y: y_min as u32,
        w: (x_max - x_min) as u32,
        h: (y_max - y_min) as u32,
    };

    let inner_effect = |x, y, im: &mut I| {
        let rgb = im.get_pixel(x, y);
        im.put_pixel(x, y, fn_inner_color(&rgb));
    };
    {
        let mut put_pixel = |c: Option<u32>, x, y| {
            if c.is_some() {
                im.put_pixel(x, y, *color);
            } else {
                inner_effect(x, y, &mut im);
            }
        };
        for x in draw_bx.x_range() {
            put_pixel(corner_1.1, x, draw_bx.y);
            put_pixel(corner_2.1, x, draw_bx.y + draw_bx.h - 1);
        }
        for y in draw_bx.y_range() {
            put_pixel(corner_1.0, draw_bx.x, y);
            put_pixel(corner_2.0, draw_bx.x + draw_bx.w - 1, y);
        }
    }
    draw_bx.effect_per_inner_pixel(|x, y| inner_effect(x, y, &mut im));
    im
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
    assert_eq!(bb.corner(1), (10, 19));
    assert_eq!(bb.corner(2), (19, 19));
    assert_eq!(bb.corner(3), (19, 10));
    assert_eq!(bb.opposite_corner(0), (19, 19));
    assert_eq!(bb.opposite_corner(1), (19, 10));
    assert_eq!(bb.opposite_corner(2), (10, 10));
    assert_eq!(bb.opposite_corner(3), (10, 19));
    for (c, i) in bb.corners().zip(0..4) {
        assert_eq!(c, bb.corner(i));
    }
    let shape = Shape::new(100, 100);
    let bb1 = bb.translate(1, 1, shape);
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
    let bb1 = bb.extend_max(1, 1, shape);
    assert_eq!(
        bb1,
        Some(BB {
            x: 10,
            y: 10,
            w: 11,
            h: 11
        })
    );
    let bb1 = bb.extend_max(100, 1, shape);
    assert_eq!(bb1, None);
    let bb1 = bb.extend_max(-1, -2, shape);
    assert_eq!(
        bb1,
        Some(BB {
            x: 10,
            y: 10,
            w: 9,
            h: 8
        })
    );
    let bb1 = bb.extend_max(-100, -200, shape);
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
