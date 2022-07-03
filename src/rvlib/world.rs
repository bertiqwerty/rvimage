use std::{fmt::Debug, mem};

use crate::result::{RvError, RvResult};
use crate::types::ViewImage;
use crate::util::{self, Shape, BB};
use image::{imageops, imageops::FilterType, DynamicImage, ImageBuffer, Rgb, Rgba};
use pixels::Pixels;
pub type AnnotationImage = ImageBuffer<Rgba<u8>, Vec<u8>>;

pub fn scaled_to_win_view(ims_raw: &ImsRaw, zoom_box: Option<BB>, shape_win: Shape) -> ViewImage {
    let shape_orig = ims_raw.shape();
    let unscaled = util::shape_unscaled(&zoom_box, shape_orig);
    let new = util::shape_scaled(unscaled, shape_win);
    let im_view = if let Some(c) = zoom_box {
        let mut ims_raw = ims_raw.clone();
        ims_raw.apply(
            |mut im| im.crop(c.x, c.y, c.w, c.h),
            |mut a| imageops::crop(&mut a, c.x, c.y, c.w, c.h).to_image(),
        );
        ims_raw.to_view()
    } else {
        ims_raw.to_view()
    };
    imageops::resize(&im_view, new.w, new.h, FilterType::Nearest)
}

fn rgba_at(i: usize, im: &ViewImage) -> [u8; 4] {
    let x = (i % im.width() as usize) as u32;
    let y = (i / im.width() as usize) as u32;
    let rgb = im.get_pixel(x, y).0;
    let rgb_changed = rgb;
    [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff]
}

fn to_01(x: u8) -> f32 {
    x as f32 / 255.0
}

fn assert_data_is_valid(shape: Shape, im_anntoations: &Option<AnnotationImage>) -> RvResult<()> {
    let shape_a = if let Some(im_a) = im_anntoations {
        Shape::from_im(im_a)
    } else {
        shape
    };
    if shape_a != shape {
        return Err(RvError::new(
            "shape mismatch between annotation and background",
        ));
    }
    Ok(())
}

fn add_annotation_to_view(
    x: u32,
    y: u32,
    im_annotation: &AnnotationImage,
    im_view: &mut ViewImage,
) {
    if im_annotation.get_pixel(x, y)[0] > 0 {
        let [r_bg, g_bg, b_bg] = im_view.get_pixel(x, y).0;
        let pixel = *im_annotation.get_pixel(x, y);
        let [r_anno, g_anno, b_anno, alpha_anno] = pixel.0;
        let alpha_amount = to_01(alpha_anno);
        let apply_alpha = |x_anno, x_res| {
            ((to_01(x_anno) * alpha_amount + (1.0 - alpha_amount) * to_01(x_res)) * 255.0) as u8
        };
        *im_view.get_pixel_mut(x, y) = Rgb([
            apply_alpha(r_anno, r_bg),
            apply_alpha(g_anno, g_bg),
            apply_alpha(b_anno, b_bg),
        ]);
    }
}
#[derive(Clone, Default, PartialEq)]
pub struct ImsRaw {
    im_background: DynamicImage,
    im_annotations: Option<AnnotationImage>,
}

impl ImsRaw {
    pub fn new(im_background: DynamicImage) -> Self {
        ImsRaw {
            im_background,
            im_annotations: None,
        }
    }

    pub fn im_background(&self) -> &DynamicImage {
        &self.im_background
    }

    pub fn has_annotations(&self) -> bool {
        self.im_annotations.is_some()
    }

    pub fn apply<
        FI: FnMut(DynamicImage) -> DynamicImage,
        FA: FnMut(AnnotationImage) -> AnnotationImage,
    >(
        &mut self,
        mut f_i: FI,
        f_a: FA,
    ) {
        self.im_background = f_i(mem::take(&mut self.im_background));
        self.im_annotations = mem::take(&mut self.im_annotations).map(f_a);

        assert_data_is_valid(self.shape(), &self.im_annotations).expect("invalid data");
    }

    pub fn set_annotations_pixel(&mut self, x: u32, y: u32, value: &[u8; 4]) {
        if let Some(im_annotations) = &mut self.im_annotations {
            *im_annotations.get_pixel_mut(x, y) = Rgba(*value);
        }
    }

    pub fn im_annotations_mut(&mut self) -> &mut Option<AnnotationImage> {
        &mut self.im_annotations
    }

    pub fn create_annotations_layer(&mut self) {
        self.im_annotations = Some(AnnotationImage::new(self.shape().w, self.shape().h));
    }

    pub fn shape(&self) -> Shape {
        Shape::from_im(&self.im_background)
    }

    pub fn to_view(&self) -> ViewImage {
        let mut im_view = util::orig_to_0_255(&self.im_background, &None);
        match &self.im_annotations {
            Some(im_a) => {
                util::effect_per_pixel(Shape::from_im(im_a), |x, y| {
                    add_annotation_to_view(x, y, im_a, &mut im_view)
                });
            }
            None => {}
        }
        im_view
    }
}

impl Debug for ImsRaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nshape {:?}\nannotations shape {:?}",
            self.shape(),
            self.im_annotations.as_ref().map(Shape::from_im),
        )
    }
}

/// Everything we need to draw
#[derive(Clone, Default)]
pub struct World {
    ims_raw: ImsRaw,
    im_view: ViewImage,
    // transforms coordinates from view to raw image
    zoom_box: Option<BB>,
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
    pub fn new(ims_raw: ImsRaw, zoom_box: Option<BB>, shape_win: Shape) -> Self {
        let im_view = scaled_to_win_view(&ims_raw, zoom_box, shape_win);
        Self {
            ims_raw,
            im_view,
            zoom_box,
        }
    }
    pub fn from_im(im: DynamicImage, shape_win: Shape) -> Self {
        Self::new(ImsRaw::new(im), None, shape_win)
    }
    pub fn set_view(&mut self, im_view: ViewImage) -> bool {
        if Shape::from_im(&im_view) == Shape::from_im(self.im_view()) {
            self.im_view = im_view;
            true
        } else {
            false
        }
    }
    pub fn im_view(&self) -> &ViewImage {
        &self.im_view
    }
    pub fn set_view_pixel(&mut self, x: u32, y: u32, value: Rgb<u8>) {
        *self.im_view.get_pixel_mut(x, y) = value;
    }
    pub fn ims_raw(&self) -> &ImsRaw {
        &self.ims_raw
    }
    pub fn ims_raw_mut(&mut self) -> &mut ImsRaw {
        &mut self.ims_raw
    }
    pub fn set_annotations_pixel(&mut self, x: u32, y: u32, value: &[u8; 4]) {
        self.ims_raw_mut().set_annotations_pixel(x, y, value);
    }
    pub fn update_view(&mut self, shape_win: Shape) {
        self.im_view = scaled_to_win_view(self.ims_raw(), *self.zoom_box(), shape_win);
    }
    pub fn shape_orig(&self) -> Shape {
        self.ims_raw.shape()
    }
    pub fn set_zoom_box(&mut self, zoom_box: Option<BB>, shape_win: Shape) {
        self.im_view = scaled_to_win_view(self.ims_raw(), zoom_box, shape_win);
        self.zoom_box = zoom_box;
    }
    pub fn zoom_box(&self) -> &Option<BB> {
        &self.zoom_box
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
use image::{GenericImage, GenericImageView};

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
    let ref_pixel = Rgba([100, 100, 100, 255]);
    ims_raw.create_annotations_layer();
    ims_raw.apply(
        |mut im: DynamicImage| {
            util::effect_per_pixel(Shape::from_im(&im), |x, y| {
                im.put_pixel(x, y, ref_pixel);
            });
            im
        },
        |mut a| {
            a.fill(26); // approx 10% alpha
            a
        },
    );
    let im_view = ims_raw.to_view();
    util::effect_per_pixel(Shape::from_im(&im_view), |x, y| {
        assert_eq!(ims_raw.im_background().get_pixel(x, y), ref_pixel);
        assert_eq!(
            ims_raw.im_annotations.as_ref().unwrap().get_pixel(x, y),
            &Rgba([26u8, 26u8, 26u8, 26u8])
        );
        // 10% fewer background image plus 10% from the annotations image
        assert_eq!(*im_view.get_pixel(x, y), Rgb([92, 92, 92]));
    });

    Ok(())
}

#[test]
fn test_scale_to_win() -> RvResult<()> {
    let mut im_test = ViewImage::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    im_test.put_pixel(10, 10, Rgb([23, 23, 23]));
    let im_scaled = scaled_to_win_view(
        &ImsRaw::new(DynamicImage::ImageRgb8(im_test)),
        None,
        Shape { w: 128, h: 128 },
    );
    assert_eq!(im_scaled.get_pixel(0, 0).0, [23, 23, 23]);
    assert_eq!(im_scaled.get_pixel(20, 20).0, [23, 23, 23]);
    assert_eq!(im_scaled.get_pixel(70, 70).0, [0, 0, 0]);
    Ok(())
}
