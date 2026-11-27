use image::{GenericImageView, ImageBuffer, Rgb, imageops};

use rvimage_domain::{BbF, BbI, Calc, PtF, ShapeI, TPtF, pos_transform};

pub type ImageU8 = ImageBuffer<Rgb<u8>, Vec<u8>>;

pub const START_WIDTH: u32 = 640;
pub const START_HEIGHT: u32 = 480;

/// Scales a coordinate from an axis of `size_from` to an axis of `size_to`
pub fn scale_coord<T>(x: T, size_from: T, size_to: T) -> T
where
    T: Calc,
{
    x * size_to / size_from
}

fn coord_view_2_orig(x: TPtF, n_transformed: TPtF, n_orig: TPtF, off: TPtF) -> TPtF {
    off + scale_coord(x, n_transformed, n_orig)
}

/// Converts the position of a pixel in the view to the coordinates of the original image
pub fn pos_2_orig_pos(
    view_pos: PtF,
    shape_orig: ShapeI,
    shape_win: ShapeI,
    zoom_box: &Option<BbF>,
) -> PtF {
    pos_transform(view_pos, shape_orig, shape_win, zoom_box, coord_view_2_orig)
}
fn coord_orig_2_view(x: f64, n_transformed: f64, n_orig: f64, off: f64) -> f64 {
    scale_coord(x - off, n_orig, n_transformed)
}

/// Converts the position of a pixel in the view to the coordinates of the original image
pub fn pos_from_orig_pos(
    orig_pos: PtF,
    shape_orig: ShapeI,
    shape_win: ShapeI,
    zoom_box: &Option<BbF>,
) -> Option<PtF> {
    if let Some(zb) = zoom_box
        && !zb.contains(orig_pos)
    {
        return None;
    }
    Some(pos_transform(
        orig_pos,
        shape_orig,
        shape_win,
        zoom_box,
        coord_orig_2_view,
    ))
}
/// The region of the original image that is visible, i.e., the zoom box snapped to pixels.
#[must_use]
pub fn crop_bb(shape_orig: ShapeI, zoom_box: Option<BbF>) -> BbI {
    if let Some(zoom_box) = zoom_box {
        let (img_w, img_h) = (shape_orig.w, shape_orig.h);
        let x = (zoom_box.x.round() as u32).min(img_w);
        let y = (zoom_box.y.round() as u32).min(img_h);
        let w = (zoom_box.w.round() as u32).min(img_w - x);
        let h = (zoom_box.h.round() as u32).min(img_h - y);
        BbI { x, y, w, h }
    } else {
        BbI {
            x: 0,
            y: 0,
            w: shape_orig.w,
            h: shape_orig.h,
        }
    }
}

#[must_use]
pub fn from_orig(im_orig: &ImageU8, zoom_box: Option<BbF>) -> ImageU8 {
    if zoom_box.is_some() {
        let crop = crop_bb(ShapeI::from_im(im_orig), zoom_box);
        im_orig.view(crop.x, crop.y, crop.w, crop.h).to_image()
    } else {
        im_orig.clone()
    }
}

/// Shape of the view image that shows `crop` with at most `max_shape` pixels per dimension.
/// The view is only downscaled, never upscaled, and preserves the aspect ratio.
#[must_use]
pub fn fit_view_shape(crop: ShapeI, max_shape: Option<ShapeI>) -> ShapeI {
    match max_shape {
        Some(max_shape) if crop.w > 0 && crop.h > 0 => {
            let scale = (f64::from(max_shape.w) / f64::from(crop.w))
                .min(f64::from(max_shape.h) / f64::from(crop.h))
                .min(1.0);
            ShapeI::new(
                ((f64::from(crop.w) * scale).ceil() as u32).clamp(1, crop.w),
                ((f64::from(crop.h) * scale).ceil() as u32).clamp(1, crop.h),
            )
        }
        _ => crop,
    }
}

/// Whether a view of shape `current` should be recomputed to get the `desired` shape. We
/// rebuild if the view is too coarse but tolerate views that are slightly too fine to avoid
/// expensive rebuilds in each frame while the window is resized.
#[must_use]
pub fn is_view_rebuild_needed(current: ShapeI, desired: ShapeI) -> bool {
    let too_coarse = desired.w > current.w || desired.h > current.h;
    let too_fine = u64::from(desired.w) * 5 < u64::from(current.w) * 4
        || u64::from(desired.h) * 5 < u64::from(current.h) * 4;
    too_coarse || too_fine
}

/// Crops the original image to `crop` and downscales it to `view_shape` if necessary.
#[must_use]
pub fn from_orig_scaled(im_orig: &ImageU8, crop: BbI, view_shape: ShapeI) -> ImageU8 {
    let sub = im_orig.view(crop.x, crop.y, crop.w, crop.h);
    if view_shape == crop.shape() {
        sub.to_image()
    } else {
        imageops::thumbnail(&*sub, view_shape.w, view_shape.h)
    }
}

/// Maps a pixel coordinate of the original image to the view pixel that displays it.
#[must_use]
pub fn orig_px_to_view_px(x: u32, crop_x: u32, crop_w: u32, view_w: u32) -> u32 {
    (u64::from(x - crop_x) * u64::from(view_w) / u64::from(crop_w)) as u32
}

#[must_use]
pub fn project_on_bb(p: PtF, bb: &BbF) -> PtF {
    let x = p.x.max(bb.x).min(bb.x + bb.w - 1.0);
    let y = p.y.max(bb.y).min(bb.y + bb.h - 1.0);
    PtF { x, y }
}

#[test]
fn test_view_scaling() {
    let crop = ShapeI::new(20000, 10000);
    assert_eq!(fit_view_shape(crop, None), crop);
    assert_eq!(
        fit_view_shape(crop, Some(ShapeI::new(2000, 2000))),
        ShapeI::new(2000, 1000)
    );
    // never upscale
    let small = ShapeI::new(50, 30);
    assert_eq!(fit_view_shape(small, Some(ShapeI::new(2000, 2000))), small);

    assert!(!is_view_rebuild_needed(
        ShapeI::new(2000, 1000),
        ShapeI::new(2000, 1000)
    ));
    assert!(is_view_rebuild_needed(
        ShapeI::new(2000, 1000),
        ShapeI::new(2001, 1000)
    ));
    assert!(!is_view_rebuild_needed(
        ShapeI::new(2000, 1000),
        ShapeI::new(1900, 950)
    ));
    assert!(is_view_rebuild_needed(
        ShapeI::new(2000, 1000),
        ShapeI::new(1000, 500)
    ));

    let mut im = ImageU8::new(40, 20);
    im.put_pixel(10, 4, Rgb([255, 255, 255]));
    let crop = crop_bb(
        ShapeI::new(40, 20),
        Some(BbF::from_arr(&[10.0, 4.0, 20.0, 10.0])),
    );
    assert_eq!(crop, BbI::from_arr(&[10, 4, 20, 10]));
    let view = from_orig_scaled(&im, crop, crop.shape());
    assert_eq!(view.get_pixel(0, 0), &Rgb([255, 255, 255]));
    let view = from_orig_scaled(&im, crop, ShapeI::new(10, 5));
    assert_eq!(ShapeI::from_im(&view), ShapeI::new(10, 5));
    // the last pixel of the crop is displayed in the last view pixel
    assert_eq!(orig_px_to_view_px(29, crop.x, crop.w, 10), 9);
    assert_eq!(orig_px_to_view_px(10, crop.x, crop.w, 10), 0);
}

#[test]
fn test_project() {
    let bb = BbF::from_arr(&[5.0, 5.0, 10.0, 10.0]);
    assert_eq!(
        PtF { x: 5.0, y: 5.0 },
        project_on_bb((0.0, 0.0).into(), &bb)
    );
    assert_eq!(
        PtF { x: 14.0, y: 14.0 },
        project_on_bb((15.0, 20.0).into(), &bb)
    );
    assert_eq!(
        PtF { x: 10.0, y: 14.0 },
        project_on_bb((10.0, 15.0).into(), &bb)
    );
    assert_eq!(
        PtF { x: 14.0, y: 14.0 },
        project_on_bb((20.0, 15.0).into(), &bb)
    );
}
