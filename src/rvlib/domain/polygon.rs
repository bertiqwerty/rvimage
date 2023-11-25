use crate::result::RvResult;

use super::{PtF, PtI, Point, Shape, OutOfBoundsMode, BB};
use super::core::max_squaredist;
use serde::{Deserialize, Serialize};

fn intersect_y_axis_parallel(lineseg: (PtF, PtF), x_value: f32) -> Option<PtF> {
    let (p1, p2) = lineseg;
    // Check if the line segment intersects with the left boundary
    if p1.x.min(p2.x) < x_value && p1.x.max(p2.x) >= x_value {
        let t = (x_value - p1.x) / (p2.x - p1.x);
        let y = p1.y + t * (p2.y - p1.y);
        Some(Point { x: x_value, y })
    } else {
        None
    }
}
fn _intersect_x_axis_parallel(lineseg: (PtF, PtF), y_value: f32) -> Option<PtF> {
    let (p1, p2) = lineseg;
    // Check if the line segment intersects with the top boundary
    if p1.y.min(p2.y) < y_value && p2.y.max(p2.y) >= y_value {
        let t = (y_value - p1.y) / (p2.y - p1.y);
        let x = p1.x + t * (p2.x - p1.x);
        Some(Point { x, y: y_value }.into())
    } else {
        None
    }
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct Polygon {
    points: Vec<PtI>, // should NEVER be empty, hence private!
    enclosing_bb: BB,
    is_open: bool,
}
impl Polygon {
    pub fn shape_check(self, orig_im_shape: Shape, mode: OutOfBoundsMode) -> Option<Self> {
        if self.enclosing_bb.contains_bb(BB::from_shape(orig_im_shape)) {
            Some(self)
        } else {
            match mode {
                OutOfBoundsMode::Deny => None,
                OutOfBoundsMode::Resize(min_bb_shape) => {
                    let shape = Shape {
                        w: orig_im_shape.w.max(min_bb_shape.w),
                        h: orig_im_shape.h.max(min_bb_shape.h),
                    };
                    let bb = BB::from_shape(shape);
                    Some(self.intersect(bb))
                }
            }
        }
    }
    pub fn min_enclosing_bb(&self) -> PtI {
        self.enclosing_bb.min()
    }
    pub fn translate(
        &self,
        _x: i32,
        _y: i32,
        _shape: Shape,
        _oob_mode: OutOfBoundsMode,
    ) -> Option<Self> {
        panic!("not implemented");
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
    pub fn distance_to_boundary(&self, _point: PtF) -> f32 {
        panic!("not implemented");
    }
    pub fn intersect(self, _other: BB) -> Self {
        panic!("not implemented");
        
         
    }
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
                if let Some(p) = intersect_y_axis_parallel((p1, p2), point.x) {
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
    pub fn from_vec(points: Vec<PtI>, is_open: bool) -> RvResult<Self> {
        let enclosing_bb = BB::from_vec(&points)?;
        Ok(Self {
            points,
            enclosing_bb,
            is_open,
        })
    }
}
impl From<BB> for Polygon {
    fn from(bb: BB) -> Self {
        Polygon {
            points: bb.points_iter().collect(),
            enclosing_bb: bb,
            is_open: false,
        }
    }
}