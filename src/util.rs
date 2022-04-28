use std::{ffi::OsStr, io};

use pixels::Pixels;

use crate::ImageType;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub w: u32,
    pub h: u32,
}

/// shape without scaling according to zoom
pub fn shape_unscaled(zoom_box: &Option<BB>, shape_orig: Shape) -> Shape {
    zoom_box.map_or(shape_orig, |z| z.shape())
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BB {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
impl BB {
    pub fn shape(&self) -> Shape {
        Shape {
            w: self.w,
            h: self.h,
        }
    }
}

pub fn shape_from_im(im: &ImageType) -> Shape {
    Shape {
        w: im.width(),
        h: im.height(),
    }
}
