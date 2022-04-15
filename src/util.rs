use std::{ffi::OsStr, io};

use pixels::Pixels;
use winit::dpi::PhysicalSize;

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
    pub h: u32
}

/// shape without scaling according to zoom
pub fn shape_unscaled(zoom_box: &Option<BB>, shape_orig: Shape) -> Shape {
    zoom_box.map_or(shape_orig, |z| Shape { w: z.w, h: z.h })
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
#[derive(Clone, Copy, Debug)]
pub struct BB {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
/// Converts the mouse position to the coordinates of the original image
pub fn mouse_pos_to_orig_pos(
    mouse_pos: Option<(usize, usize)>,
    shape_orig: Shape,
    size_win: &PhysicalSize<u32>,
    zoom_box: &Option<BB>,
) -> Option<(u32, u32)> {
    let unscaled = shape_unscaled(zoom_box, shape_orig);
    let orig = shape_orig;
    let scaled = shape_scaled(
        unscaled,
        Shape {
            w: size_win.width,
            h: size_win.height,
        },
    );

    let (x_off, y_off) = match zoom_box {
        Some(c) => (c.x, c.y),
        _ => (0, 0),
    };

    let coord_trans_2_orig = |x: u32, n_transformed: u32, n_orig: u32| -> u32 {
        (x as f64 / n_transformed as f64 * n_orig as f64) as u32
    };

    match mouse_pos {
        Some((x, y)) => {
            let x_orig = x_off + coord_trans_2_orig(x as u32, scaled.w, unscaled.w);
            let y_orig = y_off + coord_trans_2_orig(y as u32, scaled.h, unscaled.h);
            if x_orig < orig.w && y_orig < orig.h {
                Some((x_orig, y_orig))
            } else {
                None
            }
        }
        _ => None,
    }
}