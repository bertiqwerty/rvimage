mod bb;
mod core;
mod polygon;

pub use bb::BB;
pub use core::{Calc, OutOfBoundsMode, Point, PtF, PtI, Shape};
pub use polygon::Polygon;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Line {
    pub points: Vec<PtI>,
}

impl Line {
    pub fn push(&mut self, p: PtI) {
        self.points.push(p);
    }
    pub fn new() -> Self {
        Self { points: vec![] }
    }
    #[allow(clippy::needless_lifetimes)]
    pub fn points_iter<'a>(&'a self) -> impl Iterator<Item = PtI> + 'a + Clone {
        self.points.iter().copied()
    }
    pub fn last_point(&self) -> Option<PtI> {
        self.points.last().copied()
    }
    pub fn max_dist_squared(&self) -> Option<u32> {
        (0..self.points.len())
            .flat_map(|i| {
                (0..self.points.len())
                    .map(|j| self.points[i].dist_square(&self.points[j]))
                    .max()
            })
            .max()
    }
    pub fn mean(&self) -> Option<PtF> {
        let n_points = self.points.len() as u32;
        if n_points == 0 {
            None
        } else {
            Some(
                PtF::from(
                    self.points_iter()
                        .fold(Point { x: 0, y: 0 }, |p1, p2| p1 + p2),
                ) / n_points as f32,
            )
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum GeoFig {
    BB(BB),
    Poly(Polygon),
}

impl GeoFig {
    pub fn contains<P>(&self, point: P) -> bool
    where
        P: Into<PtF>,
    {
        match self {
            Self::BB(bb) => bb.contains(point),
            Self::Poly(poly) => poly.contains(point),
        }
    }
    pub fn distance_to_boundary(&self, point: PtF) -> f32 {
        match self {
            Self::BB(bb) => bb.distance_to_boundary(point),
            Self::Poly(poly) => poly.distance_to_boundary(point),
        }
    }
    pub fn is_contained_in_image(&self, shape: Shape) -> bool {
        match self {
            Self::BB(bb) => bb.is_contained_in_image(shape),
            Self::Poly(poly) => poly.is_contained_in_image(shape),
        }
    }
    pub fn max_squaredist(&self, other: &Self) -> (PtI, PtI, i64) {
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
    pub fn has_overlap(&self, other: &BB) -> bool {
        match self {
            Self::BB(bb) => bb.has_overlap(other),
            Self::Poly(poly) => poly.has_overlap(other),
        }
    }
    pub fn translate(self, p: Point<i32>, shape: Shape, oob_mode: OutOfBoundsMode) -> Option<Self> {
        match self {
            Self::BB(bb) => bb.translate(p.x, p.y, shape, oob_mode).map(GeoFig::BB),
            Self::Poly(poly) => poly.translate(p.x, p.y, shape, oob_mode).map(GeoFig::Poly),
        }
    }
    pub fn enclosing_bb(&self) -> BB {
        match self {
            Self::BB(bb) => *bb,
            Self::Poly(poly) => poly.enclosing_bb(),
        }
    }
    pub fn follow_movement(
        self,
        from: PtF,
        to: PtF,
        shape: Shape,
        oob_mode: OutOfBoundsMode,
    ) -> Option<Self> {
        let x_shift: i32 = (to.x - from.x) as i32;
        let y_shift: i32 = (to.y - from.y) as i32;
        self.translate(
            Point {
                x: x_shift,
                y: y_shift,
            },
            shape,
            oob_mode,
        )
    }

    pub fn points_normalized(&self, w: f32, h: f32) -> Vec<PtF> {
        fn convert(iter: impl Iterator<Item = PtI>, w: f32, h: f32) -> Vec<PtF> {
            iter.map(<PtI as Into<PtF>>::into)
                .map(|p| Point {
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
        Self::BB(BB::default())
    }
}

pub fn zoom_box_mouse_wheel(zoom_box: Option<BB>, shape_orig: Shape, y_delta: f32) -> Option<BB> {
    let current_zb = if let Some(zb) = zoom_box {
        zb
    } else {
        BB::from_arr(&[0, 0, shape_orig.w, shape_orig.h])
    };
    let clip_val = 1.0;
    let y_delta_clipped = if y_delta > 0.0 {
        y_delta.min(clip_val)
    } else {
        y_delta.max(-clip_val)
    };
    let factor = 1.0 - y_delta_clipped * 0.1;

    Some(current_zb.center_scale(factor, shape_orig))
}

/// shape of the image that fits into the window
pub fn shape_scaled(shape_unscaled: Shape, shape_win: Shape) -> (f32, f32) {
    let w_ratio = shape_unscaled.w as f32 / shape_win.w as f32;
    let h_ratio = shape_unscaled.h as f32 / shape_win.h as f32;
    let ratio = w_ratio.max(h_ratio);
    let w_new = shape_unscaled.w as f32 / ratio;
    let h_new = shape_unscaled.h as f32 / ratio;
    (w_new, h_new)
}
/// shape without scaling to window
pub fn shape_unscaled(zoom_box: &Option<BB>, shape_orig: Shape) -> Shape {
    zoom_box.map_or(shape_orig, |z| z.shape())
}
pub fn pos_transform<F>(
    pos: PtF,
    shape_orig: Shape,
    shape_win: Shape,
    zoom_box: &Option<BB>,
    transform: F,
) -> PtF
where
    F: Fn(f32, f32, f32, f32) -> f32,
{
    let unscaled = shape_unscaled(zoom_box, shape_orig);
    let (w_scaled, h_scaled) = shape_scaled(unscaled, shape_win);

    let (x_off, y_off) = match zoom_box {
        Some(c) => (c.x, c.y),
        _ => (0, 0),
    };

    let (x, y) = pos.into();
    let x_tf = transform(x, w_scaled, unscaled.w as f32, x_off as f32);
    let y_tf = transform(y, h_scaled, unscaled.h as f32, y_off as f32);
    (x_tf, y_tf).into()
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
#[cfg(test)]
pub fn make_test_geos() -> Vec<GeoFig> {
    make_test_bbs()
        .into_iter()
        .map(|bb| GeoFig::BB(bb))
        .collect()
}

#[test]
fn test_polygon() {
    let bbs = make_test_bbs();
    let poly = Polygon::from(bbs[2]);
    assert_eq!(poly.enclosing_bb(), bbs[2]);
    let corners = bbs[0].points_iter().collect::<Vec<_>>();
    let ebb = BB::from_vec(&corners).unwrap();
    let poly = Polygon::from(ebb);
    assert_eq!(poly.enclosing_bb(), ebb);
}

#[test]
fn test_zb() {
    fn test(zb: Option<BB>, y_delta: f32, reference_coords: &[u32; 4]) {
        println!("y_delta {}", y_delta);
        let shape = Shape::new(200, 100);
        let zb_new = zoom_box_mouse_wheel(zb, shape, y_delta);
        assert_eq!(zb_new, Some(BB::from_arr(reference_coords)));
    }
    test(None, 1.0, &[10, 5, 180, 90]);
    test(None, -1.0, &[0, 0, 200, 100]);
}

#[test]
fn test_bb() {
    let bb = BB {
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
    let shape = Shape::new(100, 100);
    let bb1 = bb.translate(1, 1, shape, OutOfBoundsMode::Deny);
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
    let bb_moved = bb
        .follow_movement(
            (5, 5).into(),
            (6, 6).into(),
            Shape::new(100, 100),
            OutOfBoundsMode::Deny,
        )
        .unwrap();
    assert_eq!(bb_moved, BB::from_arr(&[11, 11, 10, 10]));
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
    let bb1 = BB::from_arr(&[5, 5, 11, 11]);
    let bb2 = BB::from_arr(&[5, 5, 11, 11]);
    assert_eq!(
        bb1.max_squaredist(bb2.points_iter()),
        ((15, 5).into(), (5, 15).into(), 200)
    );
    let bb2 = BB::from_arr(&[6, 5, 11, 11]);
    assert_eq!(
        bb1.max_squaredist(bb2.points_iter()),
        ((5, 15).into(), (16, 5).into(), 221)
    );
    let bb2 = BB::from_arr(&[15, 15, 11, 11]);
    assert_eq!(
        bb1.max_squaredist(bb2.points_iter()),
        ((5, 5).into(), (25, 25).into(), 800)
    );
}

#[test]
fn test_intersect() {
    let bb = BB::from_arr(&[10, 15, 20, 10]);
    assert_eq!(bb.intersect(bb), bb);
    assert_eq!(
        bb.intersect(BB::from_arr(&[5, 7, 10, 10])),
        BB::from_arr(&[10, 15, 5, 2])
    );
    assert_eq!(bb.intersect_or_self(None), bb);
    assert_eq!(
        bb.intersect_or_self(Some(BB::from_arr(&[5, 7, 10, 10]))),
        BB::from_arr(&[10, 15, 5, 2])
    );
}

#[test]
fn test_into() {
    let pt: PtI = (10, 20).into();
    assert_eq!(pt, PtI { x: 10, y: 20 });
    let pt: PtF = (10i32, 20i32).into();
    assert_eq!(pt, PtF { x: 10.0, y: 20.0 });
}
