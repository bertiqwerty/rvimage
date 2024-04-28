mod bb;
mod canvas;
mod core;
mod line;
mod polygon;
pub mod result;
pub use bb::{BbF, BbI, BbS, BB};
pub use canvas::{
    access_mask_abs, access_mask_rel, canvases_to_image, mask_to_rle, rle_bb_to_image,
    rle_image_to_bb, rle_to_mask, Canvas,
};
pub use core::{
    color_with_intensity, dist_lineseg_point, max_from_partial, min_from_partial, Calc, Circle,
    CoordinateBox, OutOfBoundsMode, Point, PtF, PtI, PtS, ShapeF, ShapeI, TPtF, TPtI, TPtS,
};
pub use line::{bresenham_iter, BrushLine, Line, RenderTargetOrShape};
pub use polygon::Polygon;
pub use result::{to_rv, RvError, RvResult};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub enum GeoFig {
    BB(BbF),
    Poly(Polygon),
}

impl GeoFig {
    pub fn max_squaredist(&self, other: &Self) -> (PtF, PtF, TPtF) {
        match self {
            Self::BB(bb) => match other {
                GeoFig::BB(bb_other) => bb.max_squaredist(bb_other.points_iter()),
                GeoFig::Poly(poly_other) => bb.max_squaredist(poly_other.points_iter()),
            },
            Self::Poly(poly) => match other {
                GeoFig::BB(bb_other) => poly.max_squaredist(bb_other.points_iter()),
                GeoFig::Poly(poly_other) => poly.max_squaredist(poly_other.points_iter()),
            },
        }
    }
    pub fn has_overlap(&self, other: &BbF) -> bool {
        match self {
            Self::BB(bb) => bb.has_overlap(other),
            Self::Poly(poly) => poly.has_overlap(other),
        }
    }
    pub fn translate(
        self,
        p: Point<f64>,
        shape: ShapeI,
        oob_mode: OutOfBoundsMode<f64>,
    ) -> Option<Self> {
        match self {
            Self::BB(bb) => bb.translate(p.x, p.y, shape, oob_mode).map(GeoFig::BB),
            Self::Poly(poly) => poly.translate(p.x, p.y, shape, oob_mode).map(GeoFig::Poly),
        }
    }
    pub fn point(&self, idx: usize) -> PtF {
        match &self {
            GeoFig::BB(bb) => bb.corner(idx),
            GeoFig::Poly(p) => p.points()[idx],
        }
    }
    pub fn follow_movement(
        self,
        from: PtF,
        to: PtF,
        shape: ShapeI,
        oob_mode: OutOfBoundsMode<f64>,
    ) -> Option<Self> {
        let x_shift = (to.x - from.x) as TPtF;
        let y_shift = (to.y - from.y) as TPtF;
        self.translate(
            Point {
                x: x_shift,
                y: y_shift,
            },
            shape,
            oob_mode,
        )
    }

    pub fn points(&self) -> Vec<PtF> {
        match self {
            GeoFig::BB(bb) => bb.points_iter().collect(),
            GeoFig::Poly(poly) => poly.points_iter().collect(),
        }
    }

    pub fn points_normalized(&self, w: f64, h: f64) -> Vec<PtF> {
        fn convert(iter: impl Iterator<Item = PtF>, w: f64, h: f64) -> Vec<PtF> {
            iter.map(|p| Point {
                x: p.x / w,
                y: p.y / h,
            })
            .collect()
        }
        match self {
            GeoFig::BB(bb) => convert(bb.points_iter(), w, h),
            GeoFig::Poly(poly) => convert(poly.points_iter(), w, h),
        }
    }
}
impl Default for GeoFig {
    fn default() -> Self {
        Self::BB(BbF::default())
    }
}

/// shape of the image that fits into the window
pub fn shape_scaled(shape_unscaled: ShapeF, shape_win: ShapeI) -> (TPtF, TPtF) {
    let w_ratio = shape_unscaled.w / shape_win.w as TPtF;
    let h_ratio = shape_unscaled.h / shape_win.h as TPtF;
    let ratio = w_ratio.max(h_ratio);
    let w_new = shape_unscaled.w as TPtF / ratio;
    let h_new = shape_unscaled.h as TPtF / ratio;
    (w_new, h_new)
}
/// shape without scaling to window
pub fn shape_unscaled(zoom_box: &Option<BbF>, shape_orig: ShapeI) -> ShapeF {
    zoom_box.map_or(shape_orig.into(), |z| z.shape())
}
pub fn pos_transform<F>(
    pos: PtF,
    shape_orig: ShapeI,
    shape_win: ShapeI,
    zoom_box: &Option<BbF>,
    transform: F,
) -> PtF
where
    F: Fn(f64, f64, f64, f64) -> f64,
{
    let unscaled = shape_unscaled(zoom_box, shape_orig);
    let (w_scaled, h_scaled) = shape_scaled(unscaled, shape_win);

    let (x_off, y_off) = match zoom_box {
        Some(c) => (c.x, c.y),
        _ => (0.0, 0.0),
    };

    let (x, y) = pos.into();
    let x_tf = transform(x, w_scaled, unscaled.w, x_off);
    let y_tf = transform(y, h_scaled, unscaled.h, y_off);
    (x_tf, y_tf).into()
}

pub fn make_test_bbs() -> Vec<BbF> {
    let boxes = [
        BbI {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        },
        BbI {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
        },
        BbI {
            x: 9,
            y: 9,
            w: 10,
            h: 10,
        },
    ];
    boxes.iter().map(|bb| (*bb).into()).collect()
}

pub fn make_test_geos() -> Vec<GeoFig> {
    make_test_bbs().into_iter().map(GeoFig::BB).collect()
}

#[test]
fn test_polygon() {
    let bbs = make_test_bbs();
    let poly = Polygon::from(bbs[2]);
    assert_eq!(poly.enclosing_bb(), bbs[2]);
    let corners = bbs[0].points_iter().collect::<Vec<_>>();
    let ebb = BbF::from_vec(&corners).unwrap();
    let poly = Polygon::from(ebb);
    assert_eq!(poly.enclosing_bb(), ebb);
}

#[test]
fn test_bb() {
    let bb = BbI {
        x: 10,
        y: 10,
        w: 10,
        h: 10,
    };
    assert!(!bb.contains((20u32, 20u32)));
    assert!(bb.contains((10u32, 10u32)));
    assert!(bb.corner(0).equals((10, 10)));
    assert!(bb.corner(1).equals((10, 19)));
    assert!(bb.corner(2).equals((19, 19)));
    assert!(bb.corner(3).equals((19, 10)));
    assert!(bb.opposite_corner(0).equals((19, 19)));
    assert!(bb.opposite_corner(1).equals((19, 10)));
    assert!(bb.opposite_corner(2).equals((10, 10)));
    assert!(bb.opposite_corner(3).equals((10, 19)));
    for (c, i) in bb.points_iter().zip(0..4) {
        assert_eq!(c, bb.corner(i));
    }
    let shape = ShapeI::new(100, 100);
    let bb = <BbI as Into<BbF>>::into(bb);
    let bb1 = bb.translate(1.0, 1.0, shape, OutOfBoundsMode::Deny);
    assert_eq!(
        bb1,
        Some(
            BbI {
                x: 11,
                y: 11,
                w: 10,
                h: 10
            }
            .into()
        )
    );
    let shape = ShapeI::new(100, 100);
    let bb1 = bb.shift_max(1.0, 1.0, shape);
    assert_eq!(
        bb1,
        Some(
            BbI {
                x: 10,
                y: 10,
                w: 11,
                h: 11
            }
            .into()
        )
    );
    let bb1 = bb.shift_max(100.0, 1.0, shape);
    assert_eq!(bb1, None);
    let bb1 = bb.shift_max(-1.0, -2.0, shape);
    assert_eq!(
        bb1,
        Some(
            BbI {
                x: 10,
                y: 10,
                w: 9,
                h: 8
            }
            .into()
        )
    );
    let bb1 = bb.shift_max(-100.0, -200.0, shape);
    assert_eq!(bb1, None);
    let bb_moved = bb
        .follow_movement(
            (5, 5).into(),
            (6, 6).into(),
            ShapeI::new(100, 100),
            OutOfBoundsMode::Deny,
        )
        .unwrap();
    assert_eq!(bb_moved, BbI::from_arr(&[11, 11, 10, 10]).into());
}

#[test]
fn test_has_overlap() {
    let bb1 = BbI::from_arr(&[5, 5, 10, 10]);
    let bb2 = BbI::from_arr(&[5, 5, 10, 10]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[0, 0, 10, 10]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[0, 0, 11, 11]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[2, 2, 5, 5]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[5, 5, 9, 9]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[7, 7, 12, 12]);
    assert!(bb1.has_overlap(&bb2) && bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[17, 17, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[17, 17, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[17, 3, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
    let bb2 = BbI::from_arr(&[3, 17, 112, 112]);
    assert!(!bb1.has_overlap(&bb2) && !bb2.has_overlap(&bb1));
}

#[test]
fn test_max_corner_dist() {
    let bb1 = BbI::from_arr(&[5, 5, 11, 11]);
    let bb2 = BbI::from_arr(&[5, 5, 11, 11]);
    assert_eq!(
        bb1.max_squaredist(bb2.points_iter()),
        ((15, 5).into(), (5, 15).into(), 200)
    );
    let bb2 = BbI::from_arr(&[6, 5, 11, 11]);
    assert_eq!(
        bb1.max_squaredist(bb2.points_iter()),
        ((5, 15).into(), (16, 5).into(), 221)
    );
    let bb2 = BbI::from_arr(&[15, 15, 11, 11]);
    assert_eq!(
        bb1.max_squaredist(bb2.points_iter()),
        ((5, 5).into(), (25, 25).into(), 800)
    );
}

#[test]
fn test_intersect() {
    let bb = BbI::from_arr(&[10, 15, 20, 10]);
    assert_eq!(bb.intersect(bb), bb);
    assert_eq!(
        bb.intersect(BbI::from_arr(&[5, 7, 10, 10])),
        BbI::from_arr(&[10, 15, 5, 2])
    );
    assert_eq!(bb.intersect_or_self(None), bb);
    assert_eq!(
        bb.intersect_or_self(Some(BbI::from_arr(&[5, 7, 10, 10]))),
        BbI::from_arr(&[10, 15, 5, 2])
    );
}

#[test]
fn test_into() {
    let pt: PtI = (10, 20).into();
    assert_eq!(pt, PtI { x: 10, y: 20 });
    let pt: PtF = (10i32, 20i32).into();
    assert_eq!(pt, PtF { x: 10.0, y: 20.0 });
    {
        let box_int = BbI::from_arr(&[1, 2, 5, 6]);
        let box_f: BbF = box_int.into();
        assert_eq!(box_int, box_f.into());
    }
    {
        let box_f = BbF::from_arr(&[23.0, 2.0, 15., 31.]);
        let box_int: BbI = box_f.into();
        assert_eq!(box_int, box_f.into())
    }
}
