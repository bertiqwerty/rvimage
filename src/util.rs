use std::{ffi::OsStr, io};

use pixels::Pixels;

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
/// Converts the mouse position to the coordinates of the original image
pub fn mouse_pos_to_orig_pos(
    mouse_pos: Option<(usize, usize)>,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> Option<(u32, u32)> {
    let unscaled = shape_unscaled(zoom_box, shape_orig);
    let orig = shape_orig;
    let scaled = shape_scaled(unscaled, shape_win);

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
        &Some(BB{ x: 10, y: 10, w: 20, h: 20}),
    );
    assert_eq!(Some((10, 10)), orig_pos);
    let orig_pos = mouse_pos_to_orig_pos(
        Some((19, 19)),
        Shape { w: 120, h: 120 },
        Shape { w: 20, h: 20 },
        &Some(BB{ x: 10, y: 10, w: 20, h: 20}),
    );
    assert_eq!(Some((29, 29)), orig_pos);
    let orig_pos = mouse_pos_to_orig_pos(
        Some((10, 10)),
        Shape { w: 120, h: 120 },
        Shape { w: 20, h: 20 },
        &Some(BB{ x: 10, y: 10, w: 20, h: 20}),
    );
    assert_eq!(Some((20, 20)), orig_pos);
}
