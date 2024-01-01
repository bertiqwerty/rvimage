use image::{GenericImageView, ImageBuffer, Rgb};

use crate::domain::{pos_transform, BbF, Calc, PtF, ShapeI, TPtF};

pub type ImageU8 = ImageBuffer<Rgb<u8>, Vec<u8>>;

/// Scales a coordinate from an axis of size_from to an axis of size_to
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
pub fn view_pos_2_orig_pos(
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
pub fn orig_pos_2_view_pos(
    orig_pos: PtF,
    shape_orig: ShapeI,
    shape_win: ShapeI,
    zoom_box: &Option<BbF>,
) -> Option<PtF> {
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
pub fn orig_2_view(im_orig: &ImageU8, zoom_box: Option<BbF>) -> ImageU8 {
    if let Some(zoom_box) = zoom_box {
        im_orig
            .view(
                zoom_box.x.round() as u32,
                zoom_box.y.round() as u32,
                zoom_box.w.round() as u32,
                zoom_box.h.round() as u32,
            )
            .to_image()
    } else {
        im_orig.clone()
    }
}

pub fn project_on_bb(p: PtF, bb: &BbF) -> PtF {
    let x = p.x.max(bb.x).min(bb.x + bb.w - 1.0);
    let y = p.y.max(bb.y).min(bb.y + bb.h - 1.0);
    PtF { x, y }
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
