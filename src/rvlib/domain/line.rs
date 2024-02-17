use image::{ImageBuffer, Pixel};
use imageproc::drawing::{draw_filled_circle_mut, BresenhamLineIter};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use tracing::warn;

use super::core::{
    color_with_intensity, dist_lineseg_point, max_from_partial, Point, PtF, ShapeI, TPtF,
};

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct BrushLine {
    pub line: Line,
    pub intensity: TPtF,
    pub thickness: TPtF,
}
impl Eq for BrushLine {}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub points: Vec<PtF>,
}

impl Line {
    pub fn push(&mut self, p: PtF) {
        self.points.push(p);
    }
    pub fn new() -> Self {
        Self { points: vec![] }
    }
    #[allow(clippy::needless_lifetimes)]
    pub fn points_iter<'a>(&'a self) -> impl Iterator<Item = PtF> + 'a + Clone {
        self.points.iter().copied()
    }
    pub fn last_point(&self) -> Option<PtF> {
        self.points.last().copied()
    }
    pub fn dist_to_point(&self, p: PtF) -> Option<f64> {
        match self.points.len().cmp(&1) {
            Ordering::Greater => (0..(self.points.len() - 1))
                .map(|i| {
                    let ls: (PtF, PtF) = (self.points[i], self.points[i + 1]);
                    dist_lineseg_point(&ls, p)
                })
                .min_by(|x, y| match x.partial_cmp(y) {
                    Some(o) => o,
                    None => {
                        warn!("NaN appeared in distance to line computation.");
                        std::cmp::Ordering::Greater
                    }
                }),
            Ordering::Equal => Some(p.dist_square(&PtF::from(self.points[0])).sqrt()),
            Ordering::Less => None,
        }
    }
    pub fn max_dist_squared(&self) -> Option<f64> {
        (0..self.points.len())
            .flat_map(|i| {
                (0..self.points.len())
                    .map(|j| self.points[i].dist_square(&self.points[j]))
                    .max_by(max_from_partial)
            })
            .max_by(max_from_partial)
    }
    pub fn mean(&self) -> Option<PtF> {
        let n_points = self.points.len() as u32;
        if n_points == 0 {
            None
        } else {
            Some(
                PtF::from(
                    self.points_iter()
                        .fold(Point { x: 0.0, y: 0.0 }, |p1, p2| p1 + p2),
                ) / n_points as f64,
            )
        }
    }
}

pub enum RenderTargetOrShape<CLR>
where
    CLR: Pixel,
{
    Image(ImageBuffer<CLR, Vec<u8>>),
    Shape(ShapeI),
}
impl<CLR> RenderTargetOrShape<CLR>
where
    CLR: Pixel<Subpixel = u8>,
{
    pub fn make_buffer(self) -> ImageBuffer<CLR, Vec<u8>> {
        match self {
            RenderTargetOrShape::Image(im) => im,
            RenderTargetOrShape::Shape(shape) => ImageBuffer::<CLR, Vec<u8>>::new(shape.w, shape.h),
        }
    }
}
pub fn bresenham_iter<'a>(
    points: impl Iterator<Item = PtF> + 'a + Clone,
) -> impl Iterator<Item = (i32, i32)> + 'a {
    let p1_iter = points.clone();
    let mut p2_iter = points;
    p2_iter.next();
    p1_iter.zip(p2_iter).flat_map(|(p1, p2)| {
        BresenhamLineIter::new((p1.x as f32, p1.y as f32), (p2.x as f32, p2.y as f32))
    })
}

pub fn render_line<'a, CLR>(
    line_points: impl Iterator<Item = PtF> + 'a + Clone,
    intensity: TPtF,
    thickness: TPtF,
    image_or_shape: RenderTargetOrShape<CLR>,
    color: CLR,
) -> ImageBuffer<CLR, Vec<u8>>
where
    CLR: Pixel<Subpixel = u8>,
{
    let mut im = image_or_shape.make_buffer();
    let color = color_with_intensity(color, intensity);
    for center in bresenham_iter(line_points) {
        draw_filled_circle_mut(&mut im, center, (thickness * 0.5) as i32, color);
    }
    im
}