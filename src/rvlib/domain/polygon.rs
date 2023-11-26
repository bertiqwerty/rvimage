use crate::result::RvResult;
use std::mem;

use super::core::max_squaredist;
use super::{OutOfBoundsMode, Point, PtF, PtI, Shape, BB};
use serde::{Deserialize, Serialize};

fn lineseg_starting(idx: usize, vertices: &[PtF]) -> (PtF, PtF) {
    if idx < vertices.len() - 1 {
        (vertices[idx], vertices[idx + 1])
    } else {
        (vertices[idx], vertices[0])
    }
}

fn dist_lineseg_point(ls: &(PtF, PtF), p: PtF) -> f32 {
    let (p1, p2) = ls;
    let p1 = *p1;
    let p2 = *p2;
    let d = (p1 - p2).len_square().sqrt();
    let n = (p1 - p2) / d;
    let proj = p1 + n * (p - p1).dot(&n);
    if proj.x >= p1.x.min(p2.x)
        && proj.x <= p1.x.max(p2.x)
        && proj.y >= p1.y.min(p2.y)
        && proj.y <= p1.y.max(p2.y)
    {
        (p - proj).len_square().sqrt()
    } else {
        (p - p1).len_square().min((p - p2).len_square()).sqrt()
    }
}

fn intersect_y_axis_parallel(lineseg: &(PtF, PtF), x_value: f32) -> Option<PtF> {
    let (p1, p2) = lineseg;
    // Check if the line segment is parallel to the x-axis
    if (p1.x - p2.x).abs() > 1e-8 && p1.x.min(p2.x) < x_value && p1.x.max(p2.x) > x_value {
        let t = (x_value - p1.x) / (p2.x - p1.x);
        let y = p1.y + t * (p2.y - p1.y);
        Some(Point { x: x_value, y })
    } else {
        None
    }
}
fn intersect_x_axis_parallel(lineseg: &(PtF, PtF), y_value: f32) -> Option<PtF> {
    let (p1, p2) = lineseg;
    // Check if the line segment is parallel to the y-axis and cuts y_value
    if (p1.y - p2.y).abs() > 1e-8 && p1.y.min(p2.y) < y_value && p1.y.max(p2.y) > y_value {
        let t = (y_value - p1.y) / (p2.y - p1.y);
        let x = p1.x + t * (p2.x - p1.x);
        Some(Point { x, y: y_value })
    } else {
        None
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct Polygon {
    points: Vec<PtI>, // should NEVER be empty, hence private!
    enclosing_bb: BB,
}
impl Polygon {
    pub fn shape_check(self, orig_im_shape: Shape, mode: OutOfBoundsMode) -> Option<Self> {
        let shape_bb = BB::from_shape(orig_im_shape);
        if shape_bb.contains_bb(self.enclosing_bb) {
            Some(self)
        } else {
            match mode {
                OutOfBoundsMode::Deny => {
                    if self.points_iter().all(|p| shape_bb.contains(p)) {
                        Some(self)
                    } else {
                        None
                    }
                }
                OutOfBoundsMode::Resize(min_bb_shape) => {
                    let shape = Shape {
                        w: orig_im_shape.w.max(min_bb_shape.w),
                        h: orig_im_shape.h.max(min_bb_shape.h),
                    };
                    let bb = BB::from_shape(shape);
                    self.intersect(bb).ok()
                }
            }
        }
    }
    pub fn min_enclosing_bb(&self) -> PtI {
        self.enclosing_bb.min()
    }
    pub fn translate(
        mut self,
        x: i32,
        y: i32,
        shape: Shape,
        oob_mode: OutOfBoundsMode,
    ) -> Option<Self> {
        for p in &mut self.points {
            p.x = (p.x as i32 + x).max(0) as u32;
            p.y = (p.y as i32 + y).max(0) as u32;
        }
        self.shape_check(shape, oob_mode)
    }
    pub fn max_squaredist(&self, other: impl Iterator<Item = PtI> + Clone) -> (PtI, PtI, i64) {
        max_squaredist(self.points_iter(), other)
    }
    #[allow(clippy::needless_lifetimes)]
    pub fn points_iter<'a>(&'a self) -> impl Iterator<Item = PtI> + 'a + Clone {
        self.points.iter().copied()
    }
    pub fn has_overlap(&self, other: &BB) -> bool {
        self.enclosing_bb.has_overlap(other)
            && (other.contains_bb(self.enclosing_bb)
                || other.points_iter().any(|p| self.contains(p)))
    }
    pub fn distance_to_boundary(&self, point: PtF) -> f32 {
        self.lineseg_iter()
            .map(|ls| {
                let (p1, p2) = ls;
                let p1 = p1.into();
                let p2 = p2.into();
                dist_lineseg_point(&(p1, p2), point)
            })
            .min_by(|x, y| {
                x.partial_cmp(y)
                    .expect("this is a bug. NaNs should not appear")
            })
            .expect("this is a bug. polygons need multiple line segments")
    }

    /// Intersects the polygon with a bounding box for rendering and cut with the zoom box.
    /// Sutherland-Hodgman algorithm where the clipping polygon is a box.
    /// https://en.wikipedia.org/wiki/Sutherland%E2%80%93Hodgman_algorithm
    pub fn intersect(self, bb: BB) -> RvResult<Self> {
        let mut in_vertices: Vec<PtF> = self.points.iter().map(|p| (*p).into()).collect();
        let mut out_vertices = vec![];
        let mut process_point = |select_coord: fn(&PtF) -> f32,
                                 intersect: fn(&(PtF, PtF), f32) -> Option<PtF>,
                                 corner,
                                 cmp: fn(f32, f32) -> bool| {
            for (idx, v) in in_vertices.iter().enumerate() {
                if cmp(select_coord(v), select_coord(&corner)) {
                    // add vertex if inside of box
                    out_vertices.push(*v);
                }

                // add intersection
                let ls = lineseg_starting(idx, &in_vertices);
                let intersp = intersect(&ls, select_coord(&corner));
                if let Some(intersp) = intersp {
                    out_vertices.push(intersp);
                }
            }
            in_vertices = mem::take(&mut out_vertices);
        };
        for (corner_idx, corner) in bb.points_iter().map(<PtI as Into<PtF>>::into).enumerate() {
            if corner_idx == 0 {
                // intersection with left line segment of bounding box
                process_point(
                    |p| p.x,
                    intersect_y_axis_parallel,
                    corner,
                    |x, xleft| x >= xleft,
                );
            } else if corner_idx == 1 {
                // intersection with left btm segment of bounding box
                process_point(
                    |p| p.y,
                    intersect_x_axis_parallel,
                    corner,
                    |y, ybtm| y <= ybtm,
                );
            } else if corner_idx == 2 {
                // intersection with left line segment of bounding box
                process_point(
                    |p| p.x,
                    intersect_y_axis_parallel,
                    corner,
                    |x, xright| x <= xright,
                );
            } else if corner_idx == 3 {
                // intersection with left btm segment of bounding box
                process_point(
                    |p| p.y,
                    intersect_x_axis_parallel,
                    corner,
                    |y, ybtm| y >= ybtm,
                );
            }
        }
        Self::from_vec(in_vertices.into_iter().map(|v| v.into()).collect())
    }
    #[allow(clippy::needless_lifetimes)]
    fn lineseg_iter<'a>(&'a self) -> impl Iterator<Item = (PtI, PtI)> + 'a {
        self.points.iter().enumerate().map(|(i, p1)| {
            let p2 = if i < self.points.len() - 1 {
                self.points[i + 1]
            } else {
                self.points[0]
            };
            ((*p1), p2)
        })
    }
    pub fn contains<P>(&self, point: P) -> bool
    where
        P: Into<PtF>,
    {
        // we will check the number of cuts from of rays from the point to the top
        // parallel to the y-axis.
        //   odd number => inside
        //   even number => outside
        let point = point.into();
        let n_cuts = self
            .lineseg_iter()
            .filter(|(p1, p2)| {
                let p1: PtF = (*p1).into();
                let p2: PtF = (*p2).into();
                if let Some(p) = intersect_y_axis_parallel(&(p1, p2), point.x) {
                    p.y >= point.y
                } else {
                    false
                }
            })
            .count();
        n_cuts % 2 == 1
    }
    pub fn is_contained_in_image(&self, shape: Shape) -> bool {
        self.enclosing_bb.is_contained_in_image(shape)
    }
    pub fn enclosing_bb(&self) -> BB {
        self.enclosing_bb
    }
    pub fn points(&self) -> &Vec<PtI> {
        &self.points
    }
    /// We will need this as soon as we support polygons
    pub fn from_vec(points: Vec<PtI>) -> RvResult<Self> {
        let enclosing_bb = BB::from_vec(&points)?;
        Ok(Self {
            points,
            enclosing_bb,
        })
    }
}
impl From<BB> for Polygon {
    fn from(bb: BB) -> Self {
        Polygon {
            points: bb.points_iter().collect(),
            enclosing_bb: bb,
        }
    }
}

#[test]
fn test_intersect() {
    let ls = ((15.0, 15.0).into(), (5.0, 15.0).into());
    let intersp = intersect_x_axis_parallel(&ls, 8.0);
    assert!(intersp.is_none());
    let ls = ((5.0, 15.0).into(), (5.0, 5.0).into());
    let intersp = intersect_x_axis_parallel(&ls, 8.0);
    if let Some(ip) = intersp {
        assert!((ip.x - 5.0).abs() < 1e-8);
        assert!((ip.y - 8.0).abs() < 1e-8);
    } else {
        assert!(false)
    }
}

#[test]
fn test_poly() {
    let poly = Polygon::from(BB::from_arr(&[5, 5, 10, 10]));
    assert!(!poly.contains(PtI::from((17, 7))));
    assert!(poly.contains(PtI::from((7, 7))));
    let bb = BB::from_arr(&[2, 2, 33, 30]);
    assert!(poly.has_overlap(&bb));
    let bb = BB::from_arr(&[6, 6, 7, 7]);
    assert!(poly.has_overlap(&bb));
    let bb = BB::from_arr(&[6, 6, 15, 15]);
    assert!(poly.has_overlap(&bb));
}
#[test]
fn test_poly_triangle() {
    let poly = Polygon::from_vec(vec![(5, 5).into(), (10, 10).into(), (5, 10).into()]).unwrap();
    assert!(poly.contains(PtI::from((6, 9))));
    assert!(!poly.contains(PtF::from((6.0, 5.99))));
    assert!(poly.contains(PtF::from((6.0, 6.01))));
}
#[test]
fn test_poly_intersect() {
    let poly = Polygon::from_vec(vec![(5, 5).into(), (15, 15).into(), (5, 15).into()]).unwrap();
    let bb = BB::from_arr(&[5, 7, 10, 2]);
    let clipped_poly = poly.clone().intersect(bb).unwrap();
    let encl_bb = BB::from_arr(&[5, 7, 4, 2]);
    assert_eq!(clipped_poly.enclosing_bb(), encl_bb);
    assert_eq!(
        clipped_poly.points,
        vec![(7, 7).into(), (8, 8).into(), (5, 8).into(), (5, 7).into()]
    );

    let bb = BB::from_arr(&[5, 7, 2, 2]);
    let clipped_poly = poly.intersect(bb);
    assert_eq!(clipped_poly.unwrap().enclosing_bb(), bb);

    let poly = Polygon::from_vec(vec![(5, 5).into(), (10, 10).into(), (5, 10).into()]).unwrap();
    let clipped_poly = poly.clone().intersect(BB::from_arr(&[2, 2, 20, 20]));
    assert_eq!(clipped_poly, Ok(poly));
}

#[test]
fn test_min_dist() {
    let poly = Polygon::from_vec(vec![(5, 5).into(), (15, 15).into(), (5, 15).into()]).unwrap();
    let p = (5, 5).into();
    let d = poly.distance_to_boundary(p).abs();
    assert!(d < 1e-8);
    let p = (0, 5).into();
    let d = poly.distance_to_boundary(p).abs();
    assert!((5.0 - d).abs() < 1e-8);
    let p = (10, 10).into();
    let d = poly.distance_to_boundary(p).abs();
    assert!(d.abs() < 1e-8);
    let p = (10, 11).into();
    let d = poly.distance_to_boundary(p).abs();
    assert!((0.5f32.sqrt() - d).abs() < 1e-8);
}
