use std::{fmt::Debug, mem};

use crate::format_rverr;
use crate::result::{RvError, RvResult};
use crate::types::ViewImage;
use crate::util::{self, Shape};
use image::{DynamicImage, ImageBuffer, Luma};
use pixels::Pixels;

pub type MaskImage = Option<ImageBuffer<Luma<u8>, Vec<u8>>>;

fn rgba_at(i: usize, im: &ViewImage) -> [u8; 4] {
    let x = (i % im.width() as usize) as u32;
    let y = (i / im.width() as usize) as u32;
    let rgb = im.get_pixel(x, y).0;
    let rgb_changed = rgb;
    [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff]
}

fn assert_data_is_valid(
    shape: Shape,
    ims_layers: &Vec<DynamicImage>,
    ims_masks: &Vec<MaskImage>,
) -> RvResult<()> {
    if ims_layers.len() != ims_masks.len() {
        Err(format_rverr!(
            "lengths of ims and masks need to coincide but {} vs. {}",
            ims_layers.len(),
            ims_masks.len()
        ))
    } else {
        for idx in 0..ims_layers.len() {
            let im_l = &ims_layers[idx];
            let im_m = &ims_masks[idx];
            let shape_m = if let Some(im_m) = im_m {
                Shape::from_im(im_m)
            } else {
                shape
            };
            if Shape::from_im(im_l) != shape || shape_m != shape {
                return Err(RvError::new("shape mismatch between layer and background"));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Default, PartialEq)]
pub struct ImsRaw {
    im_background: DynamicImage,
    ims_layers: Vec<DynamicImage>,
    ims_masks: Vec<MaskImage>,
}

impl ImsRaw {
    pub fn new(im_background: DynamicImage) -> Self {
        ImsRaw {
            im_background,
            ims_layers: vec![],
            ims_masks: vec![],
        }
    }

    pub fn im_background(&self) -> &DynamicImage {
        &self.im_background
    }

    pub fn from_multiple_layers(
        im_background: DynamicImage,
        ims_layers: Vec<DynamicImage>,
        ims_masks: Vec<MaskImage>,
    ) -> RvResult<Self> {
        let mut ims_raw = Self::new(im_background);
        ims_raw.append(ims_layers, ims_masks)?;
        Ok(ims_raw)
    }

    pub fn apply<FI: FnMut(DynamicImage) -> DynamicImage, FM: FnMut(MaskImage) -> MaskImage>(
        &mut self,
        mut f_i: FI,
        f_m: FM,
    ) {
        self.im_background = f_i(mem::take(&mut self.im_background));
        self.ims_layers = mem::take(&mut self.ims_layers)
            .into_iter()
            .map(f_i)
            .collect();
        self.ims_masks = mem::take(&mut self.ims_masks)
            .into_iter()
            .map(f_m)
            .collect();

        assert_data_is_valid(self.shape(), &self.ims_layers, &self.ims_masks)
            .expect("invalid data");
    }

    pub fn append(
        &mut self,
        mut ims_layers: Vec<DynamicImage>,
        mut ims_masks: Vec<MaskImage>,
    ) -> RvResult<()> {
        assert_data_is_valid(self.shape(), &ims_layers, &ims_masks)?;
        self.ims_layers.append(&mut ims_layers);
        self.ims_masks.append(&mut ims_masks);
        Ok(())
    }
    pub fn shape(&self) -> Shape {
        Shape::from_im(&self.im_background)
    }

    pub fn to_view(&self) -> ViewImage {
        let mut res = util::orig_to_0_255(&self.im_background, &None);
        for idx in 0..self.ims_layers.len() {
            let (im_raw, im_mask) = (&self.ims_layers[idx], &self.ims_masks[idx]);
            let im_transformed = util::orig_to_0_255(im_raw, im_mask);
            match im_mask {
                Some(im_m) => {
                    for y in 0..im_m.height() {
                        for x in 0..im_m.width() {
                            if im_m.get_pixel(x, y)[0] > 0 {
                                *res.get_pixel_mut(x, y) = *im_transformed.get_pixel(x, y);
                            }
                        }
                    }
                }
                None => {
                    res = im_transformed;
                }
            }
        }
        res
    }
}

impl Debug for ImsRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nshape {:?}\nnumber of layers {:?}\nnumber of masks {:?}",
            self.shape(),
            self.ims_layers.len(),
            self.ims_masks.len()
        )
    }
}

/// Everything we need to draw
#[derive(Clone, Default)]
pub struct World {
    ims_raw: ImsRaw,
    im_view: ViewImage,
}

impl World {
    pub fn draw(&self, pixels: &mut Pixels) {
        let frame_len = pixels.get_frame().len() as u32;
        let w_view = self.im_view().width();
        let h_view = self.im_view().height();
        if frame_len != w_view * h_view * 4 {
            pixels.resize_buffer(w_view, h_view);
        }
        let frame = pixels.get_frame();

        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let rgba = rgba_at(i, self.im_view());
            pixel.copy_from_slice(&rgba);
        }
    }
    pub fn new(ims_raw: ImsRaw) -> Self {
        let im_view = ims_raw.to_view();
        Self { ims_raw, im_view }
    }
    pub fn from_im(im: DynamicImage) -> Self {
        Self::new(ImsRaw::new(im))
    }
    pub fn im_view(&self) -> &ViewImage {
        &self.im_view
    }
    pub fn im_view_mut(&mut self) -> &mut ViewImage {
        &mut self.im_view
    }
    pub fn ims_raw(&self) -> &ImsRaw {
        &self.ims_raw
    }
    pub fn ims_raw_mut(&mut self) -> &mut ImsRaw {
        &mut self.ims_raw
    }
    pub fn shape_orig(&self) -> Shape {
        self.ims_raw.shape()
    }
}
impl Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nims_raw {:?}\nim_view shape {:?}",
            self.ims_raw(),
            Shape::from_im(&self.im_view)
        )
    }
}

#[cfg(test)]
use image::{GenericImage, GenericImageView, Rgb, Rgba};

#[test]
fn test_rgba() {
    let mut im_test = ViewImage::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    assert_eq!(rgba_at(0, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(0, 1, Rgb([23, 23, 23]));
    assert_eq!(rgba_at(64, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(7, 11, Rgb([23, 23, 23]));
    assert_eq!(rgba_at(11 * 64 + 7, &im_test), [23, 23, 23, 255]);
}

#[test]
fn test_ims_raw() -> RvResult<()> {
    let im = DynamicImage::ImageRgb8(ViewImage::new(64, 64));
    let mut ims_raw = ImsRaw::new(im.clone());
    let ref_pixel = Rgba([122, 122, 122, 255]);
    ims_raw.append(
        vec![im],
        vec![Some(ImageBuffer::<Luma<u8>, Vec<u8>>::new(64, 64))],
    )?;
    ims_raw.apply(
        |mut im: DynamicImage| {
            for y in 0..im.height() {
                for x in 0..im.width() {
                    im.put_pixel(x, y, ref_pixel);
                }
            }
            im
        },
        |x| {
            x.map(|mut m| {
                m.fill(11);
                m
            })
        },
    );

    for y in 0..ims_raw.shape().h {
        for x in 0..ims_raw.shape().w {
            assert_eq!(ims_raw.ims_layers[0].get_pixel(x, y), ref_pixel);
            assert_eq!(ims_raw.im_background().get_pixel(x, y), ref_pixel);
            assert_eq!(
                ims_raw.ims_masks[0].as_ref().unwrap().get_pixel(x, y),
                &Luma([11u8])
            );
        }
    }
    ims_raw.ims_layers[0].put_pixel(4, 4, Rgba([1, 1, 1, 255]));
    ims_raw.ims_layers[0].put_pixel(5, 5, Rgba([13, 13, 123, 123]));
    ims_raw.ims_masks[0]
        .as_mut()
        .unwrap()
        .put_pixel(5, 5, Luma([0]));
    let im_view = ims_raw.to_view();
    for y in 0..ims_raw.shape().h {
        for x in 0..ims_raw.shape().w {
            if x == 4 && y == 4 {
                assert_eq!(im_view.get_pixel(x, y), &Rgb([1, 1, 1]));
            } else {
                assert_eq!(im_view.get_pixel(x, y), &Rgb([122, 122, 122]));
            }
        }
    }
    Ok(())
}
