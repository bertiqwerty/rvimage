use core::cmp::Ordering::{Greater, Less};
use std::ops::{Add, Sub};

use image::{buffer::ConvertBuffer, DynamicImage, GenericImage, ImageBuffer, Luma, Rgb, Rgba};

use crate::{
    domain::{Shape, BB},
    file_util::PixelEffect,
    format_rverr,
    result::to_rv,
    types::{ResultImage, ViewImage},
};

pub fn read_image(path: &str) -> ResultImage {
    image::io::Reader::open(path)
        .map_err(to_rv)?
        .with_guessed_format()
        .map_err(to_rv)?
        .decode()
        .map_err(|e| format_rverr!("could not decode image {:?}. {:?}", path, e))
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
