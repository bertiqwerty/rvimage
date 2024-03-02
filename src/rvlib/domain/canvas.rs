use image::{ImageBuffer, Luma, Pixel};
use imageproc::drawing::draw_filled_circle_mut;
use serde::{ser::SerializeStruct, Deserialize, Serialize};

use crate::{color_with_intensity, domain::OutOfBoundsMode, result::RvResult, rverr, ShapeI};

use super::{bb::BB, line::render_line, BbI, BrushLine, PtF, PtI, RenderTargetOrShape, TPtF, TPtI};

fn line_to_mask(line: &BrushLine, orig_shape: Option<ShapeI>) -> RvResult<(Vec<u8>, BbI)> {
    let thickness = line.thickness;
    let thickness_half = thickness * 0.5;
    let bb = BB::from_points_iter(line.line.points_iter())?;

    let xywh = [
        bb.x - thickness_half,
        bb.y - thickness_half,
        bb.w + thickness,
        bb.h + thickness,
    ];

    let bb = match orig_shape {
        Some(orig_shape) => BB::new_shape_checked(
            xywh[0],
            xywh[1],
            xywh[2],
            xywh[3],
            orig_shape,
            OutOfBoundsMode::Resize(bb.shape()),
        )
        .ok_or_else(|| rverr!("Could not create bounding box for line"))?,
        None => BB::from_arr(&xywh),
    };

    let color = Luma([1]);
    let bbi = BbI::from(bb);
    let im = if line.line.points.len() == 1 {
        let mut im = RenderTargetOrShape::Shape(bbi.shape()).make_buffer();
        let center = PtF {
            x: line.line.points[0].x - bb.x,
            y: line.line.points[0].y - bb.y,
        }
        .round_signed();
        draw_filled_circle_mut(
            &mut im,
            (center.x, center.y),
            thickness_half.round() as i32,
            color,
        );
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

pub fn mask_to_rle(mask: &[u8], w: u32, h: u32) -> Vec<u32> {
    let mut rle = Vec::new();
    let mut current_run = 0;
    let mut current_value = 0;
    for y in 0..h {
        for x in 0..w {
            let value = mask[(y * w + x) as usize];
            if value == current_value {
                current_run += 1;
            } else {
                rle.push(current_run);
                current_run = 1;
                current_value = value;
            }
        }
    }
    rle.push(current_run);
    rle
}

pub fn rle_to_mask(rle: &[u32], w: u32, h: u32) -> Vec<u8> {
    let mut mask = vec![0; (w * h) as usize];
    for (i, &run) in rle.iter().enumerate() {
        let value = i % 2;
        let start = rle.iter().take(i).sum::<u32>();
        for idx in start..(start + run) {
            let x = idx % w;
            let y = idx / w;
            mask[(y * w + x) as usize] = value as u8;
        }
    }
    mask
}

fn idx_bb_to_im(idx_bb: u32, bb: BbI, w_im: TPtI) -> u32 {
    let p_im = PtI {
        y: idx_bb / bb.w,
        x: idx_bb % bb.w,
    } + bb.min();
    p_im.y * w_im + p_im.x
}

fn idx_im_to_bb(idx_im: u32, bb: BbI, w_im: TPtI) -> u32 {
    let p_bb = PtI {
        x: idx_im % w_im,
        y: idx_im / w_im,
    } - bb.min();
    p_bb.y * bb.w + p_bb.x
}
/// The input rle is computed with respect to the bounding box coordinates
/// the result is with respect to image coordinates
pub fn rle_bb_to_image(rle_bb: &[u32], bb: BbI, shape_im: ShapeI) -> RvResult<Vec<u32>> {
    if !bb.is_contained_in_image(shape_im) {
        Err(rverr!(
            "Bounding box {} is not contained in image with shape {:?}",
            bb,
            shape_im
        ))
    } else {
        let mut rle_im = vec![0; rle_bb.len()];
        let offset = idx_bb_to_im(0, bb, shape_im.w);
        rle_im[0] = offset + rle_bb[0];
        let mut prev_idx = rle_im[0];
        for i in 1..rle_bb.len() {
            let im_idx = idx_bb_to_im(rle_bb[..=i].iter().sum(), bb, shape_im.w);
            rle_im[i] = im_idx - prev_idx;
            prev_idx = im_idx;
        }
        Ok(rle_im)
    }
}
/// The input rle is computed with respect to the image coordinates
/// the result is with respect to bounding box coordinates
pub fn rle_image_to_bb(rle_im: &[u32], bb: BbI, shape_im: ShapeI) -> RvResult<Vec<u32>> {
    if !bb.is_contained_in_image(shape_im) {
        Err(rverr!(
            "Bounding box {} is not contained in image with shape {:?}",
            bb,
            shape_im
        ))
    } else {
        let mut rle_bb = vec![0; rle_im.len()];
        let offset = idx_bb_to_im(0, bb, shape_im.w);
        rle_bb[0] = rle_im[0] - offset;
        let mut prev_idx = rle_bb[0];
        for i in 1..rle_im.len() {
            let bb_idx = idx_im_to_bb(rle_im[..=i].iter().sum(), bb, shape_im.w);
            rle_bb[i] = bb_idx - prev_idx;
            prev_idx = bb_idx;
        }
        Ok(rle_bb)
    }
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

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    pub mask: Vec<u8>,
    pub bb: BbI,
    pub intensity: TPtF,
}

impl Canvas {
    pub fn new(line: &BrushLine, orig_shape: ShapeI) -> RvResult<Self> {
        let (mask, bb) = line_to_mask(line, Some(orig_shape))?;
        Ok(Self {
            mask,
            bb,
            intensity: line.intensity,
        })
    }
    /// This function does check the for out of bounds. We assume valid data has been serialized.
    pub fn from_serialized_brush_line(bl: &BrushLine) -> RvResult<Self> {
        let (mask, bb) = line_to_mask(bl, None)?;
        Ok(Self {
            mask,
            bb,
            intensity: bl.intensity,
        })
    }
}

impl Serialize for Canvas {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Canvas", 3)?;
        state.serialize_field("rle", &mask_to_rle(&self.mask, self.bb.w, self.bb.h))?;
        state.serialize_field("bb", &self.bb)?;
        state.serialize_field("intensity", &self.intensity)?;
        state.end()
    }
}
impl<'de> Deserialize<'de> for Canvas {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct CanvasDe {
            rle: Vec<u32>,
            bb: BbI,
            intensity: TPtF,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CanvasOrBl {
            Canvas(CanvasDe),
            BrushLine(BrushLine),
        }
        let read = CanvasOrBl::deserialize(deserializer)?;
        match read {
            CanvasOrBl::Canvas(canvas_de) => {
                let mask = rle_to_mask(&canvas_de.rle, canvas_de.bb.w, canvas_de.bb.h);
                Ok(Self {
                    mask,
                    bb: canvas_de.bb,
                    intensity: canvas_de.intensity,
                })
            }
            CanvasOrBl::BrushLine(bl) => {
                Canvas::from_serialized_brush_line(&bl).map_err(serde::de::Error::custom)
            }
        }
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

#[test]
fn test_rle() {
    let mask = vec![0, 0, 0, 0, 0, 0, 0, 0, 0];
    let rle = mask_to_rle(&mask, 3, 3);
    assert_eq!(rle, vec![9]);
    let mask2 = rle_to_mask(&rle, 3, 3);
    assert_eq!(mask, mask2);

    let mask = vec![1, 1, 1, 1, 1, 1, 1, 1, 1];
    let rle = mask_to_rle(&mask, 3, 3);
    assert_eq!(rle, vec![0, 9]);
    let mask2 = rle_to_mask(&rle, 3, 3);
    assert_eq!(mask, mask2);

    let mask = vec![1, 0, 0, 1, 1, 1, 0, 0, 0];
    let rle = mask_to_rle(&mask, 3, 3);
    assert_eq!(rle, vec![0, 1, 2, 3, 3]);
    let mask2 = rle_to_mask(&rle, 3, 3);
    assert_eq!(mask, mask2);

    let bb = BbI::from_arr(&[5, 10, 4, 8]);
    let shape_im = ShapeI::new(100, 200);
    let x = idx_bb_to_im(0, bb, shape_im.w);
    assert_eq!(x, 1005);
    let x = idx_bb_to_im(1, bb, shape_im.w);
    assert_eq!(x, 1006);
    let x = idx_bb_to_im(3, bb, shape_im.w);
    assert_eq!(x, 1008);
    let rle_bb = vec![1, 2, 3, 1, 4];
    let rle_im = rle_bb_to_image(&rle_bb, bb, shape_im).unwrap();
    assert_eq!(rle_im, vec![1006, 2, 99, 1, 100]);
    let rle_bb2 = rle_image_to_bb(&rle_im, bb, shape_im).unwrap();
    assert_eq!(rle_bb, rle_bb2);
}

#[test]
fn test_canvas_serde() {
    let orig_shape = ShapeI::new(30, 30);
    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 5.0, y: 5.0 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    let cv = Canvas::new(&bl, orig_shape).unwrap();
    let s = serde_json::to_string(&cv).unwrap();
    let cv_read: Canvas = serde_json::from_str(&s).unwrap();
    assert_eq!(cv, cv_read);
}
