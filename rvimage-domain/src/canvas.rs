use image::{ImageBuffer, Luma, Pixel};
use imageproc::drawing::draw_filled_circle_mut;
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use std::mem;

use crate::{color_with_intensity, result::RvResult, rverr, OutOfBoundsMode, ShapeI};

use super::{
    line::render_line, BbF, BbI, BrushLine, Point, PtF, PtI, RenderTargetOrShape, TPtF, TPtI,
};

fn line_to_mask(
    line: &BrushLine,
    orig_shape: Option<ShapeI>,
    buffer: Option<Vec<u8>>,
) -> RvResult<(Vec<u8>, BbI)> {
    let bb = line.bb(orig_shape)?;
    let color = Luma([1]);
    let bbi = BbI::from_arr(&[
        bb.x as u32,
        bb.y as u32,
        bb.w.ceil() as u32,
        bb.h.ceil() as u32,
    ]);
    let is_none = buffer.is_none();
    let mut buffer = if let Some(mut buffer) =
        buffer.filter(|buffer| buffer.len() >= (bbi.w * bbi.h) as usize)
    {
        buffer.fill(0);
        ImageBuffer::from_vec(bbi.w, bbi.h, buffer)
            .unwrap_or_else(|| RenderTargetOrShape::Shape(bbi.shape()).make_buffer())
    } else {
        tracing::debug!(
            "(re-)creating buffer, buffer is {}",
            if is_none { "None" } else { "Some" }
        );
        RenderTargetOrShape::Shape(bbi.shape()).make_buffer()
    };
    let im = if line.line.points.len() == 1 {
        let center = Point {
            x: (line.line.points[0].x - bb.x) as i32,
            y: (line.line.points[0].y - bb.y) as i32,
        };

        let thickness_half = line.thickness * 0.5;
        if line.thickness <= 1.1 {
            buffer.put_pixel(center.x as u32, center.y as u32, color);
        } else {
            let r = if thickness_half.floor() == thickness_half {
                (thickness_half - 1.0) as i32
            } else {
                thickness_half as i32
            };
            draw_filled_circle_mut(&mut buffer, (center.x, center.y), r, color);
        }
        buffer
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
            RenderTargetOrShape::Image(buffer),
            color,
        )
    };
    Ok((im.to_vec(), bbi))
}

#[must_use]
pub fn mask_to_rle(mask: &[u8], mask_w: u32, mask_h: u32) -> Vec<u32> {
    let mut rle = Vec::new();
    let mut current_run = 0;
    let mut current_value = 0;
    for y in 0..mask_h {
        for x in 0..mask_w {
            let value = mask[(y * mask_w + x) as usize];
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

pub fn rle_to_mask_inplace(rle: &[u32], mask: &mut [u8], w: u32) {
    for (i, &run) in rle.iter().enumerate() {
        let value = i % 2;
        let start = rle.iter().take(i).sum::<u32>();
        for idx in start..(start + run) {
            let x = idx % w;
            let y = idx / w;
            let idx = (y * w + x) as usize;
            if idx < mask.len() {
                mask[idx] = value as u8;
            }
        }
    }
}

#[must_use]
pub fn rle_to_mask(rle: &[u32], w: u32, h: u32) -> Vec<u8> {
    let mut mask = vec![0; (w * h) as usize];
    rle_to_mask_inplace(rle, &mut mask, w);
    mask
}

fn idx_bb_to_pixim(idx_bb: u32, bb: BbI) -> PtI {
    PtI {
        y: idx_bb / bb.w,
        x: idx_bb % bb.w,
    } + bb.min()
}

fn idx_bb_to_im(idx_bb: u32, bb: BbI, w_im: TPtI) -> u32 {
    let p_im = idx_bb_to_pixim(idx_bb, bb);
    p_im.y * w_im + p_im.x
}

fn idx_im_to_bb(idx_im: u32, bb: BbI, w_im: TPtI) -> Option<u32> {
    let p_im = PtI {
        x: idx_im % w_im,
        y: idx_im / w_im,
    };
    if bb.contains(p_im) {
        let p = p_im - bb.min();
        Some(p.y * bb.w + p.x)
    } else {
        None
    }
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
        // degenerate cases with all zeros
        if rle_bb.len() == 1 {
            return Ok(vec![shape_im.w * shape_im.h]);
        }
        // or leading rows with complete zeros
        let n_zero_rows = rle_bb[0] / bb.w;
        let bb = BbI::from_arr(&[bb.x, bb.y + n_zero_rows, bb.w, bb.h - n_zero_rows]);
        let rle_0_correction = n_zero_rows * bb.w;
        // or zeros at the end
        let n_zero_rows = if rle_bb.len() % 2 == 1 {
            rle_bb.iter().last().unwrap() / bb.w
        } else {
            0
        };
        let bb = BbI::from_arr(&[bb.x, bb.y, bb.w, bb.h - n_zero_rows]);
        let rle_1_correction = n_zero_rows * bb.w;

        let mut rle_im = vec![];
        let offset = idx_bb_to_im(0, bb, shape_im.w);
        rle_im.push(offset + rle_bb[0] - rle_0_correction);
        let mut prev_idx = rle_im[0] - 1;
        for i in 1..rle_bb.len() {
            let sum_correction = rle_0_correction
                + if i == rle_bb.len() - 1 {
                    rle_1_correction
                } else {
                    0
                };
            let im_idx = idx_bb_to_im(
                rle_bb[..=i].iter().sum::<u32>() - 1 - sum_correction,
                bb,
                shape_im.w,
            );
            let p = PtI {
                x: im_idx % shape_im.w,
                y: im_idx / shape_im.w,
            };
            let p_prev = PtI {
                x: prev_idx % shape_im.w,
                y: prev_idx / shape_im.w,
            };
            let is_foreground_run = i % 2 == 1;
            let row_span = p.y - p_prev.y;
            if is_foreground_run {
                if row_span == 0 {
                    rle_im.push(p.x - p_prev.x);
                } else {
                    let n_elts = bb.max().x - p_prev.x;
                    // in case of complete zero rows this can be zero
                    if n_elts > 0 {
                        rle_im.push(n_elts);
                        for _ in 0..(row_span - 1) {
                            rle_im.push(shape_im.w - bb.w);
                            rle_im.push(bb.w);
                        }
                        rle_im.push(shape_im.w - bb.w);
                    }
                    rle_im.push(p.x + 1 - bb.x);
                }
                if i == rle_bb.len() - 1 {
                    rle_im.push(
                        bb.x + bb.w - 1 - p.x + shape_im.w * (shape_im.h - p.y - 1) + shape_im.w
                            - (bb.w + bb.x),
                    );
                }
            } else {
                let n_elts = if row_span == 0 {
                    p.x - p_prev.x
                } else {
                    bb.x_max() + 1 - p_prev.x + (row_span - 1) * shape_im.w + shape_im.w - bb.w
                        + p.x
                        - bb.x
                };
                let n_elts = if p.x == bb.x_max() && i < rle_bb.len() - 1 {
                    n_elts + shape_im.w - bb.w
                } else {
                    n_elts
                };
                let n_elts = if i == rle_bb.len() - 1 {
                    n_elts + shape_im.w - (bb.w + bb.x) + shape_im.w * (shape_im.h - p.y - 1)
                } else {
                    n_elts
                };
                rle_im.push(n_elts);
            }
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
        // degenerate cases with all zeros
        if rle_im.len() == 1 {
            return Ok(vec![bb.w * bb.h]);
        }
        let mut mask = vec![0; (bb.w * bb.h) as usize];

        for (i, run) in rle_im.iter().enumerate() {
            let is_foreground_run = i % 2 == 1;
            if is_foreground_run {
                let start = rle_im.iter().take(i).sum::<u32>();
                for idx in start..(start + run) {
                    if let Some(idx_bb) = idx_im_to_bb(idx, bb, shape_im.w) {
                        mask[idx_bb as usize] = 1;
                    }
                }
            }
        }
        Ok(mask_to_rle(&mask, bb.w, bb.h))
    }
}

/// Get the 1d-index inside a bounding box from image coordinates
pub fn access_bb_idx(bb: BbI, p: PtI) -> usize {
    if bb.contains(p) {
        ((p.y - bb.y) * bb.w + p.x - bb.x) as usize
    } else {
        0
    }
}

/// Access a mask with coordinates for the image containing the mask
#[must_use]
pub fn access_mask_abs(mask: &[u8], bb: BbI, p: PtI) -> u8 {
    if bb.contains(p) {
        mask[access_bb_idx(bb, p)]
    } else {
        0
    }
}
#[must_use]
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
    pub fn from_line_extended(
        line: &BrushLine,
        orig_shape: ShapeI,
        extension_factor: f64,
        lower_buffer_bound: usize,
    ) -> RvResult<Self> {
        if extension_factor < 1.0 {
            return Err(rverr!("extension factor {extension_factor} smaller 1"));
        }
        let bb = line.bb(Some(orig_shape))?;
        let new_w = (bb.w * extension_factor).ceil() as usize;
        let new_h = (bb.h * extension_factor).ceil() as usize;
        let new_size = (new_w * new_h)
            .min(orig_shape.w as usize * orig_shape.h as usize)
            .max(lower_buffer_bound);
        let buffer = vec![0; new_size];
        Self::new(line, orig_shape, Some(buffer))
    }
    pub fn new(line: &BrushLine, orig_shape: ShapeI, buffer: Option<Vec<u8>>) -> RvResult<Self> {
        let (mask, bb) = line_to_mask(line, Some(orig_shape), buffer)?;
        Ok(Self {
            mask,
            bb,
            intensity: line.intensity,
        })
    }
    pub fn from_box(bb: BbI, intensity: TPtF) -> Self {
        Self {
            bb,
            mask: vec![1; (bb.w * bb.h) as usize],
            intensity,
        }
    }
    #[must_use]
    pub fn merge(mut self, other: &Canvas) -> Self {
        let old_self_bb = self.bb;
        self.bb = self.bb.merge(other.bb);
        self.mask.resize((self.bb.w * self.bb.h) as usize, 0);
        // move self-mask to new positions
        for y in (0..old_self_bb.h).rev() {
            for x in (0..old_self_bb.w).rev() {
                let p = PtI { x, y } + old_self_bb.min();
                let old_idx = (y * old_self_bb.w + x) as usize;
                let new_idx = access_bb_idx(self.bb, p);
                let val = self.mask[old_idx];
                self.mask[old_idx] = 0;
                self.mask[new_idx] = val;
            }
        }
        // incorporate the other mask
        for y in 0..other.bb.h {
            for x in 0..other.bb.w {
                let p = PtI { x, y } + other.bb.min();
                let val_self = access_mask_abs(&self.mask, self.bb, p);
                let val_other = other.mask[(y * other.bb.w + x) as usize];
                let val = val_self.max(val_other);
                self.mask[((p.y - self.bb.y) * self.bb.w + (p.x - self.bb.x)) as usize] = val;
            }
        }
        self.intensity = self.intensity.max(other.intensity);
        self
    }
    pub fn draw_circle(&mut self, center: PtF, thickness: TPtF, color: u8) -> RvResult<()> {
        let im = ImageBuffer::<Luma<u8>, Vec<u8>>::from_vec(
            self.bb.w,
            self.bb.h,
            mem::take(&mut self.mask),
        );
        if let Some(mut im) = im {
            let color = Luma([color]);
            let center = Point {
                x: (center.x - TPtF::from(self.bb.x)) as i32,
                y: (center.y - TPtF::from(self.bb.y)) as i32,
            };

            if thickness <= 1.1 {
                im.put_pixel(center.x as u32, center.y as u32, color);
            } else {
                draw_filled_circle_mut(
                    &mut im,
                    (center.x, center.y),
                    (thickness * 0.5) as i32,
                    color,
                );
            }
            self.mask = im.into_vec();
            Ok(())
        } else {
            Err(rverr!(
                "Could not create image buffer for canvas at {:?}",
                self.bb
            ))
        }
    }
    /// This function does check the for out of bounds. We assume valid data has been serialized.
    pub fn from_serialized_brush_line(bl: &BrushLine) -> RvResult<Self> {
        let (mask, bb) = line_to_mask(bl, None, None)?;
        Ok(Self {
            mask,
            bb,
            intensity: bl.intensity,
        })
    }
    pub fn follow_movement(&mut self, from: PtF, to: PtF, shape: ShapeI) {
        let x_shift = (to.x - from.x) as TPtF;
        let y_shift = (to.y - from.y) as TPtF;
        let bb: BbF = self.bb.into();
        let bb = bb.translate(x_shift, y_shift, shape, OutOfBoundsMode::Deny);
        if let Some(bb) = bb {
            self.bb = bb.into();
        }
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
use super::{Line, BB};
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
    let cv = Canvas::new(&bl, orig_shape, None).unwrap();
    assert!(cv.mask.iter().sum::<u8>() > 0);
    let buffer = vec![43; 100];
    let orig_shape = ShapeI::new(30, 30);
    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 5.0, y: 5.0 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    let cv2 = Canvas::new(&bl, orig_shape, Some(buffer)).unwrap();
    assert_eq!(cv.mask.iter().sum::<u8>(), cv2.mask.iter().sum::<u8>());
}

#[test]
fn test_rle() {
    fn test(bb: BbI, shape: ShapeI, rle_bb: &[u32], rle_im_ref: &[u32], skip_rec: bool) {
        let rle_im = rle_bb_to_image(rle_bb, bb, shape).unwrap();
        assert_eq!(rle_im, rle_im_ref);
        assert_eq!(rle_im.iter().sum::<u32>(), shape.w * shape.h);
        let rle_bb_rec = rle_image_to_bb(&rle_im, bb, shape).unwrap();
        if !skip_rec {
            assert_eq!(rle_bb_rec, rle_bb);
        }
    }
    let rle_bb = [1, 1, 4, 1, 1];
    let bb = BbI::from_arr(&[1, 1, 2, 4]);
    let shape = ShapeI::new(4, 6);
    let rle_im_ref = [6, 1, 10, 1, 6];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let rle_bb = [0, 3, 1, 2];
    let bb = BbI::from_arr(&[3, 2, 2, 3]);
    let shape = ShapeI::new(6, 6);
    let rle_im_ref = [15, 2, 4, 1, 5, 2, 7];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let rle_bb = [0, 1, 3];
    let bb = BbI::from_arr(&[2, 2, 2, 2]);
    let shape = ShapeI::new(6, 6);
    let rle_im_ref = [14, 1, 21];
    test(bb, shape, &rle_bb, &rle_im_ref, true);

    let rle_bb = [1, 2, 1];
    let bb = BbI::from_arr(&[1, 1, 2, 2]);
    let shape = ShapeI::new(6, 4);
    let rle_im_ref = [8, 1, 4, 1, 10];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let rle_bb = vec![0, 2, 2, 2];
    let bb = BbI::from_arr(&[3, 2, 2, 3]);
    let shape = ShapeI::new(6, 6);
    let rle_im_ref = vec![15, 2, 10, 2, 7];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let rle_bb = vec![3, 1];
    let bb = BbI::from_arr(&[1, 1, 2, 2]);
    let shape = ShapeI::new(6, 6);
    let rle_im_ref = vec![14, 1, 21];
    test(bb, shape, &rle_bb, &rle_im_ref, true);

    let rle_bb = vec![6];
    let bb = BbI::from_arr(&[2, 1, 2, 3]);
    let shape = ShapeI::new(6, 6);
    let rle_im_ref = vec![36];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let rle_bb = vec![0, 6];
    let bb = BbI::from_arr(&[2, 1, 2, 3]);
    let shape = ShapeI::new(6, 6);
    let rle_im_ref = vec![8, 2, 4, 2, 4, 2, 14];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let rle_bb = vec![0, 1, 2, 1];
    let bb = BbI::from_arr(&[1, 1, 2, 2]);
    let shape = ShapeI::new(6, 8);
    let rle_im_ref = vec![7, 1, 6, 1, 33];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let rle_bb = vec![1, 4, 1];
    let bb = BbI::from_arr(&[1, 1, 2, 3]);
    let shape = ShapeI::new(6, 6);
    let rle_im_ref = vec![8, 1, 4, 2, 4, 1, 16];
    test(bb, shape, &rle_bb, &rle_im_ref, false);

    let mask = vec![0, 1, 0, 0, 0, 0, 1, 0];
    let rle = mask_to_rle(&mask, 2, 4);
    assert_eq!(rle, vec![1, 1, 4, 1, 1]);

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
    let cv = Canvas::new(&bl, orig_shape, None).unwrap();
    let s = serde_json::to_string(&cv).unwrap();
    let cv_read: Canvas = serde_json::from_str(&s).unwrap();
    assert_eq!(cv, cv_read);
}

#[test]
fn test_line_to_mask() {
    fn test(mask_zeros: &[u8], mask_sum: u8, bb: BbI, bl: &BrushLine) {
        let (mask2, bb2) = line_to_mask(bl, None, None).unwrap();

        assert_eq!(bb, bb2);
        assert_eq!(mask2.iter().sum::<u8>(), mask_sum);
        for i in mask_zeros {
            assert_eq!(mask2[*i as usize], 0);
        }
    }

    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 4.7, y: 4.7 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    test(&[0, 2, 6, 8], 5, BB::from_arr(&[3, 3, 3, 3]), &bl);

    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 5.3, y: 5.3 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    test(&[0, 2, 6, 8], 5, BB::from_arr(&[3, 3, 3, 3]), &bl);
    let bl = BrushLine {
        line: Line {
            points: vec![PtF { x: 5.0, y: 5.0 }],
        },
        intensity: 0.5,
        thickness: 3.0,
    };
    test(&[0, 2, 6, 8], 5, BB::from_arr(&[3, 3, 3, 3]), &bl);
    let center = PtF { x: 5.0, y: 5.0 };
    let bl = BrushLine {
        line: Line {
            points: vec![center],
        },
        intensity: 0.5,
        thickness: 5.0,
    };
    test(&[], 21, BB::from_arr(&[2, 2, 5, 5]), &bl);
    let mut canvas = Canvas::new(&bl, ShapeI::new(30, 30), None).unwrap();
    canvas.draw_circle(center, 5.0, 0).unwrap();
    // maybe we didn't delete all but a significant portion due to rounding errors
    assert!(canvas.mask.iter().sum::<u8>() < 21 / 2);
}

#[test]
fn test_merge() {
    let c1 = Canvas {
        bb: BbI::from_arr(&[0, 0, 2, 2]),
        mask: vec![1, 0, 0, 1],
        intensity: 0.5,
    };
    let c2 = Canvas {
        bb: BbI::from_arr(&[0, 0, 2, 2]),
        mask: vec![1, 0, 0, 1],
        intensity: 0.7,
    };
    let merged = c1.clone().merge(&c2);
    assert_eq!(merged.mask, c1.mask);
    assert_eq!(merged.bb, c1.bb);
    assert_eq!(merged.intensity, c2.intensity);
    let c1 = Canvas {
        bb: BbI::from_arr(&[0, 0, 2, 2]),
        mask: vec![0, 1, 1, 0],
        intensity: 0.5,
    };
    let c2 = Canvas {
        bb: BbI::from_arr(&[0, 0, 2, 2]),
        mask: vec![1, 0, 0, 1],
        intensity: 0.7,
    };
    let merged = c1.merge(&c2);
    assert_eq!(merged.mask, vec![1, 1, 1, 1]);

    let c1 = Canvas {
        bb: BbI::from_arr(&[0, 0, 3, 2]),
        mask: vec![1, 0, 0, 0, 1, 0],
        intensity: 0.5,
    };
    let c2 = Canvas {
        bb: BbI::from_arr(&[2, 2, 2, 2]),
        mask: vec![1, 1, 1, 1],
        intensity: 0.7,
    };
    let merged = c1.merge(&c2);
    assert_eq!(c2.intensity, merged.intensity);
    assert_eq!(merged.mask.len(), 16);
    assert_eq!(merged.bb, BbI::from_arr(&[0, 0, 4, 4]));
    let mask_reference = vec![1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1];
    assert_eq!(merged.mask, mask_reference);

    let c1 = Canvas {
        bb: BbI::from_arr(&[1, 1, 3, 2]),
        mask: vec![1, 0, 0, 0, 1, 0],
        intensity: 0.5,
    };
    let c2 = Canvas {
        bb: BbI::from_arr(&[3, 3, 2, 2]),
        mask: vec![1, 1, 1, 1],
        intensity: 0.7,
    };
    let merged = c1.merge(&c2);
    assert_eq!(c2.intensity, merged.intensity);
    assert_eq!(merged.mask.len(), 16);
    assert_eq!(merged.bb, BbI::from_arr(&[1, 1, 4, 4]));
    let mask_reference = vec![1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1];
    assert_eq!(merged.mask, mask_reference);
}

#[test]
fn test_from_box() {
    let bb = BbI::from_arr(&[7, 8, 2, 2]);
    let i = 1.0;
    let c = Canvas::from_box(bb, i);
    assert_eq!(c.mask.len(), (bb.w * bb.h) as usize);
    assert_eq!(c.bb, bb);
    assert_eq!(c.intensity, i);
}
