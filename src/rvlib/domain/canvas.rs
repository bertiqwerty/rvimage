use image::{ImageBuffer, Luma, Pixel};
use imageproc::drawing::draw_filled_circle_mut;
use serde::{Deserialize, Serialize};

use crate::{color_with_intensity, domain::OutOfBoundsMode, result::RvResult, rverr, ShapeI};

use super::{
    bb::BB, line::render_line, BbI, BrushLine, InstanceAnnotate, PtF, PtI, RenderTargetOrShape,
    TPtF,
};

fn line_to_mask(line: &BrushLine, orig_shape: ShapeI) -> RvResult<(Vec<u8>, BbI)> {
    let thickness = line.thickness;
    let thickness_half = thickness * 0.5;
    let bb = BB::from_points_iter(line.line.points_iter())?;
    let bb = BB::new_shape_checked(
        bb.x - thickness_half,
        bb.y - thickness_half,
        bb.w + thickness,
        bb.h + thickness,
        orig_shape,
        OutOfBoundsMode::Resize(bb.shape()),
    )
    .ok_or_else(|| rverr!("Could not create bounding box for line"))?;
    let color = Luma([1]);
    let bbi = BbI::from(bb);
    let im = if line.line.points.len() == 1 {
        let mut im = RenderTargetOrShape::Shape(bbi.shape()).make_buffer();
        let center = PtF {
            x: line.line.points[0].x - bb.x,
            y: line.line.points[0].y - bb.y,
        }
        .round_signed();
        draw_filled_circle_mut(&mut im, (center.x, center.y), thickness_half.round() as i32, color);
        im
    } else {
        render_line(
            line.line
                .points_iter()
                .filter(|p| bb.contains(*p))
                .map(|p| PtF {
                    x: p.x - bb.x,
                    y: p.y - bb.y,
                }),
            1.0,
            line.thickness,
            RenderTargetOrShape::Shape(bbi.shape()),
            color,
        )
    };
    Ok((im.to_vec(), bbi))
}
/// Access a with coordinates for the image containing the mask
pub fn access_mask_abs(mask: &[u8], bb: BbI, p: PtI) -> u8 {
    if bb.contains(p) {
        mask[((p.y - bb.y) * bb.w + p.x - bb.x) as usize]
    } else {
        0
    }
}
pub fn access_mask_rel(mask: &[u8], x: u32, y: u32, w: u32, h: u32) -> u8 {
    if x < w && y < h {
        mask[(y * w + x) as usize]
    } else {
        0
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    pub mask: Vec<u8>,
    pub bb: BbI,
    pub intensity: TPtF,
}

impl Canvas {
    pub fn new(line: &BrushLine, orig_shape: ShapeI) -> RvResult<Self> {
        let (mask, bb) = line_to_mask(line, orig_shape)?;
        Ok(Self {
            mask,
            bb,
            intensity: line.intensity,
        })
    }
}

impl InstanceAnnotate for Canvas {
    fn is_contained_in_image(&self, shape: crate::ShapeI) -> bool {
        self.bb.is_contained_in_image(shape)
    }
    fn contains<P>(&self, point: P) -> bool
    where
        P: Into<super::PtF>,
    {
        let p_tmp: PtF = point.into();
        let p_idx: PtI = p_tmp.into();
        access_mask_abs(&self.mask, self.bb, p_idx) > 0
    }
    fn dist_to_boundary(&self, p: PtF) -> TPtF {
        let mut min_dist = TPtF::MAX;
        for y in 0..self.bb.h {
            for x in 0..self.bb.w {
                let is_current_foreground = access_mask_rel(&self.mask, x, y, self.bb.w, self.bb.h);
                let neighbors_fg_mask = [
                    access_mask_rel(&self.mask, x + 1, y, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x.wrapping_sub(1), y, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x, y + 1, self.bb.w, self.bb.h),
                    access_mask_rel(&self.mask, x, y.wrapping_sub(1), self.bb.w, self.bb.h),
                ];
                if neighbors_fg_mask
                    .iter()
                    .any(|&b| b != is_current_foreground)
                {
                    let x = x as TPtF;
                    let y = y as TPtF;
                    let dist = p.dist_square(&PtF { x, y }).sqrt();
                    if dist < min_dist {
                        min_dist = dist;
                    }
                }
            }
        }
        min_dist
    }
}

pub fn canvases_to_image<'a, CLR>(
    canvases: impl Iterator<Item = &'a Canvas>,
    image_or_shape: RenderTargetOrShape<CLR>,
    color: CLR,
) -> ImageBuffer<CLR, Vec<u8>>
where
    CLR: Pixel<Subpixel = u8>,
{
    let mut im = image_or_shape.make_buffer();
    for cv in canvases {
        let color = color_with_intensity(color, cv.intensity);
        for y in cv.bb.y_range() {
            for x in cv.bb.x_range() {
                let p_idx = PtI { x, y };
                let is_fg = access_mask_abs(&cv.mask, cv.bb, p_idx) > 0;
                if is_fg {
                    im.put_pixel(x, y, color);
                }
            }
        }
    }
    im
}

#[cfg(test)]
use super::Line;
#[test]
fn test_canvas() {
    let orig_shape = ShapeI::new(30, 30);
    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 5.0, y: 5.0 }, PtF { x: 15.0, y: 15.0 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    let cv = Canvas::new(&bl, orig_shape).unwrap();
    assert!(cv.contains(PtF { x: 5.0, y: 5.0 }));
    assert!(!cv.contains(PtF { x: 0.0, y: 0.0 }));
    assert!(cv.contains(PtF { x: 14.9, y: 14.9 }));
    assert!(!cv.contains(PtF { x: 0.0, y: 9.9 }));
    assert!(!cv.contains(PtF { x: 15.0, y: 15.0 }));

    assert!((cv.dist_to_boundary(PtF { x: 5.0, y: 5.0 }) - 1.0).abs() < 1e-8);
    let dist = cv.dist_to_boundary(PtF { x: 5.0, y: 15.0 });
    assert!(5.0 < dist && dist < 7.0);
    for y in cv.bb.y_range() {
        for x in cv.bb.x_range() {
            access_mask_abs(&cv.mask, cv.bb, PtI { x, y });
        }
    }
}
#[test]
fn test_canvas_single() {
    let orig_shape = ShapeI::new(30, 30);
    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 5.0, y: 5.0 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    let cv = Canvas::new(&bl, orig_shape).unwrap();
    assert!(cv.mask.iter().sum::<u8>() > 0)
}
