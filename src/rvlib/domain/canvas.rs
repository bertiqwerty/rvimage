use image::{ImageBuffer, Luma, Pixel};
use serde::{Deserialize, Serialize};

use crate::{color_with_intensity, result::RvResult};

use super::{
    bb::BB, line::render_line, BbI, BrushLine, InstanceAnnotate, PtF, PtI, RenderTargetOrShape,
    TPtF,
};

fn line_to_mask(line: &BrushLine, bb: BbI) -> Vec<u8> {
    let im = render_line(
        &line.line,
        1.0,
        line.thickness,
        RenderTargetOrShape::Shape(bb.shape()),
        Luma([1]),
    );
    im.to_vec()
}
/// Access a with coordinates for the image containing the mask
pub fn access_mask_abs(mask: &[u8], bb: BbI, p: PtI) -> u8 {
    if bb.contains(p) {
        mask[(p.y * bb.w + p.x) as usize]
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
    pub fn new(line: &BrushLine) -> RvResult<Self> {
        let bb = BB::from_points_iter(line.line.points_iter())?.into();
        let mask = vec![];
        let mut cv = Self {
            mask,
            bb,
            intensity: line.intensity,
        };
        cv.mask = line_to_mask(line, cv.bb);
        Ok(cv)
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
        if self.bb.contains(p_idx) {
            access_mask_abs(&self.mask, self.bb, p_idx) > 0
        } else {
            false
        }
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
    let mut im = match image_or_shape {
        RenderTargetOrShape::Image(im) => im,
        RenderTargetOrShape::Shape(shape) => ImageBuffer::<CLR, Vec<u8>>::new(shape.w, shape.h),
    };
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
    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 0.0, y: 0.0 }, PtF { x: 10.0, y: 10.0 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    let cv = Canvas::new(&bl).unwrap();
    assert!(cv.contains(PtF { x: 0.0, y: 0.0 }));
    assert!(cv.contains(PtF { x: 5.0, y: 5.0 }));
    assert!(cv.contains(PtF { x: 9.9, y: 9.9 }));
    assert!(!cv.contains(PtF { x: 10.0, y: 0.0 }));

    assert!((cv.dist_to_boundary(PtF { x: 5.0, y: 5.0 }) - 1.0).abs() < 1e-8);
    let dist = cv.dist_to_boundary(PtF { x: 0.0, y: 10.0 });
    assert!((dist - 5.656854249492381).abs() < 1e-8);
}
