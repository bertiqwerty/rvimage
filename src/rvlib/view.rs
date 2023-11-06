use std::ops::{Add, Div, Mul};

use image::{GenericImageView, ImageBuffer, Rgb};

use crate::domain::{pos_transform, Point, Shape, BB};

pub type ImageU8 = ImageBuffer<Rgb<u8>, Vec<u8>>;

/// Scales a coordinate from an axis of size_from to an axis of size_to
pub fn scale_coord<T>(x: T, size_from: T, size_to: T) -> T
where
    T: Mul<Output = T> + Div<Output = T> + Add<Output = T>,
{
    x * size_to / size_from
}

fn coord_view_2_orig(x: u32, n_transformed: u32, n_orig: u32, off: u32) -> u32 {
    off + scale_coord(x, n_transformed, n_orig)
}

/// Converts the position of a pixel in the view to the coordinates of the original image
pub fn view_pos_2_orig_pos(
    view_pos: Point,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> Point {
    pos_transform(view_pos, shape_orig, shape_win, zoom_box, coord_view_2_orig)
}
fn coord_orig_2_view(x: u32, n_transformed: u32, n_orig: u32, off: u32) -> u32 {
    scale_coord(x - off, n_orig, n_transformed)
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
pub fn orig_pos_2_view_pos(
    orig_pos: Point,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
) -> Option<Point> {
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
pub fn orig_2_view(im_orig: &ImageU8, zoom_box: Option<BB>) -> ImageU8 {
    if let Some(zoom_box) = zoom_box {
        im_orig
            .view(zoom_box.x, zoom_box.y, zoom_box.w, zoom_box.h)
            .to_image()
    } else {
        im_orig.clone()
    }
}
