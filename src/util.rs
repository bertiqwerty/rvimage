use std::{ffi::OsStr, io, path::Path};

use pixels::Pixels;
use winit::dpi::PhysicalSize;
use winit_input_helper::WinitInputHelper;

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

pub fn is_relative(path: &str) -> bool {
    Path::new(path).is_relative() && path.chars().next() != Some('/')
}

pub fn mouse_pos_transform(
    pixels: &Pixels,
    input_pos: Option<(f32, f32)>,
) -> Option<(usize, usize)> {
    pixels
        .window_pos_to_pixel(input_pos.unwrap_or((-1.0, -1.0)))
        .ok()
}

#[derive(Clone)]
pub struct Event<'a> {
    pub input: &'a WinitInputHelper,
    pub image_loaded: bool,
    pub window_resized: bool,
}
impl<'a> Event<'a> {
    pub fn new(input: &'a WinitInputHelper) -> Self {
        Self {
            input,
            image_loaded: false,
            window_resized: false,
        }
    }
    pub fn from_window_resized(input: &'a WinitInputHelper) -> Self {
        Self {
            input,
            image_loaded: false,
            window_resized: true,
        }
    }
    pub fn from_image_loaded(input: &'a WinitInputHelper) -> Self {
        Self {
            input,
            image_loaded: true,
            window_resized: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub w: u32,
    pub h: u32,
}
impl Shape {
    pub fn from_im(im: &ImageType) -> Self {
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
