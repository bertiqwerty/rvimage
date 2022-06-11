use crate::format_rverr;
use crate::result::RvError;
use crate::util::{self, Shape};
use crate::{result::RvResult, types::ViewImage};
use image::DynamicImage;
use pixels::Pixels;
fn rgba_at(i: usize, im: &ViewImage) -> [u8; 4] {
    let x = (i % im.width() as usize) as u32;
    let y = (i / im.width() as usize) as u32;
    let rgb = im.get_pixel(x, y).0;
    let rgb_changed = rgb;
    [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff]
}

/// Everything we need to draw
#[derive(Default)]
pub struct World {
    im_orig: DynamicImage,
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
            let rgba = rgba_at(i, &self.im_view());
            pixel.copy_from_slice(&rgba);
        }
    }
    pub fn new(im_orig: DynamicImage) -> RvResult<Self> {
        let im_view = util::orig_to_0_255(&im_orig)?;
        Ok(Self { im_orig, im_view })
    }
    pub fn im_view(&self) -> &ViewImage {
        &self.im_view
    }
    pub fn im_view_mut(&mut self) -> &mut ViewImage {
        &mut self.im_view
    }
    pub fn im_orig(&self) -> &DynamicImage {
        &self.im_orig
    }
    pub fn im_orig_mut(&mut self) -> &mut DynamicImage {
        &mut self.im_orig
    }
    pub fn shape_orig(&self) -> Shape {
        Shape {
            w: self.im_orig().width(),
            h: self.im_orig().height(),
        }
    }
}
#[cfg(test)]
use image::Rgb;
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
