use image::GenericImage;
use pixels::Pixels;
use serde::{Deserialize, Serialize};
use winit::dpi::PhysicalSize;

use std::{fmt::Display, iter::once, ops::Range, str::FromStr};

use crate::{
    file_util::PixelEffect,
    result::{to_rv, RvError},
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
            1 => (x, y + h),
            2 => (x + w, y + h),
            3 => (x + w, y),
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
    pub fn center_f(&self) -> (f32, f32) {
        (
            self.w as f32 * 0.5 + self.x as f32,
            self.h as f32 * 0.5 + self.y as f32,
        )
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
    pub fn is_contained_in_image(&self, shape: Shape) -> bool {
        self.x + self.w < shape.w && self.y + self.h < shape.h
    }

    pub fn new_shape_checked(x: i32, y: i32, w: i32, h: i32, shape: Shape) -> Option<Self> {
        if x < 0 || y < 0 || w < 1 || h < 1 {
            None
        } else {
            let bb = Self {
                x: x as u32,
                y: y as u32,
                w: w as u32,
                h: h as u32,
            };
            if bb.is_contained_in_image(shape) {
                Some(bb)
            } else {
                None
            }
        }
    }

    pub fn translate(&self, x_shift: i32, y_shift: i32, shape: Shape) -> Option<Self> {
        let x = self.x as i32 + x_shift;
        let y = self.y as i32 + y_shift;
        Self::new_shape_checked(x, y, self.w as i32, self.h as i32, shape)
    }

    pub fn new_fit_to_image(x: i32, y: i32, w: i32, h: i32, shape: Shape) -> BB {
        let x = x.max(0);
        let y = y.max(0);
        let w = (w + x).min(shape.w as i32) - x;
        let h = (h + y).min(shape.h as i32) - y;
        BB::from_arr(&[x as u32, y as u32, w as u32, h as u32])
    }

    pub fn translate_and_center_scale(
        &self,
        x_shift: f32,
        y_shift: f32,
        factor: f32,
        shape: Shape,
    ) -> Self {
        let x = self.x as f32 + x_shift;
        let y = self.y as f32 + y_shift;
        let w = self.w as f32;
        let h = self.h as f32;
        let (cx, cy) = (w * 0.5 + x, h * 0.5 + y);
        let topleft = (cx + factor * (x - cx), cy + factor * (y - cy));
        let btmright = (cx + factor * ((x + w) - cx), cy + factor * ((y + h) - cy));
        println!("cx {:?}", cx);
        println!("cy {:?}", cy);
        println!("topleft {:?}", topleft);
        println!("btmright {:?}", btmright);
        let (x_tl, y_tl) = topleft;
        let (x_br, y_br) = btmright;
        let w = (x_br - x_tl) as i32;
        let h = (y_br - y_tl) as i32;
        let x = x_tl as i32;
        let y = y_tl as i32;

        Self::new_fit_to_image(x, y, w, h, shape)
    }

    pub fn shift_max(&self, x_shift: i32, y_shift: i32, shape: Shape) -> Option<Self> {
        let (w, h) = (self.w as i32 + x_shift, self.h as i32 + y_shift);
        Self::new_shape_checked(self.x as i32, self.y as i32, w, h, shape)
    }

    pub fn shift_min(&self, x_shift: i32, y_shift: i32, shape: Shape) -> Option<Self> {
        let (x, y) = (self.x as i32 + x_shift, self.y as i32 + y_shift);
        let (w, h) = (self.w as i32 - x_shift, self.h as i32 - y_shift);
        Self::new_shape_checked(x, y, w, h, shape)
    }

    pub fn has_overlap(&self, other: &BB) -> bool {
        if self.corners().any(|c| other.contains(c)) {
            true
        } else {
            other.corners().any(|c| self.contains(c))
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
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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

pub fn zoom_box_around_mouse(
    zoom_box: Option<BB>,
    mouse: Option<(f32, f32)>,
    shape_win: Shape,
    y_delta: f32,
) -> Option<BB> {
    if let Some(mp) = mouse {
        let y_delta = if y_delta < 0.0 {
            1.0 / (1.0 - y_delta)
        } else {
            y_delta + 1.0
        };
        let current_zb = if let Some(zb) = zoom_box {
            zb
        } else {
            BB::from_arr(&[0, 0, shape_win.w, shape_win.h])
        };
        let (cx, cy) = current_zb.center_f();
        let x_shift = mp.0 - cx;
        let y_shift = mp.1 - cy;

        let current_zb =
            current_zb.translate_and_center_scale(x_shift, y_shift, y_delta, shape_win);
        Some(current_zb)
    } else {
        zoom_box
    }
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

fn test_zb() {
    fn test(zb: Option<BB>, mouse: Option<(f32, f32)>, y_delta: f32, bb_coords: &[u32; 4]) {
        let shape = Shape::new(200, 100);
        let zb_new = zoom_box_around_mouse(zb, mouse, shape, y_delta);
        assert_eq!(zb_new, Some(BB::from_arr(bb_coords)));
    }
    test(None, Some((100.0, 50.0)), -1.0, &[50, 25, 100, 50]);
    test(None, Some((100.0, 50.0)), 1.0, &[0, 0, 200, 100]);
    test(
        Some(BB::from_arr(&[0, 0, 200, 100])),
        Some((100.0, 50.0)),
        -1.0,
        &[50, 25, 100, 50],
    );
    test(None, Some((50.0, 25.0)), -1.0, &[0, 0, 100, 50]);
    test(
        Some(BB::from_arr(&[50, 25, 100, 50])),
        Some((150.0, 75.0)),
        -1.0,
        &[125, 62, 50, 25],
    );
    test(
        Some(BB::from_arr(&[0, 0, 100, 50])),
        Some((100.0, 50.0)),
        1.0,
        &[0, 0, 200, 100],
    );
    test(
        Some(BB::from_arr(&[0, 0, 100, 50])),
        Some((100.0, 50.0)),
        4.0,
        &[0, 0, 200, 100],
    );
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
