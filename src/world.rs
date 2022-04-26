use crate::tools::{Tool, ToolWrapper};
use crate::util::{mouse_pos_transform, Shape, shape_from_im};
use crate::{apply_tool_method, ImageType};
use pixels::Pixels;
use winit_input_helper::WinitInputHelper;

/// Draw the image to the frame buffer.
///
/// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
fn pixels_rgba_at(i: usize, im_view: &ImageType) -> [u8; 4] {
    let x = (i % im_view.width() as usize) as u32;
    let y = (i / im_view.width() as usize) as u32;
    let rgb = im_view.get_pixel(x, y).0;
    let rgb_changed = rgb;
    [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff]
}
/// Everything we need to draw
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct World {
    im_orig: ImageType,
    im_view: ImageType,
}

impl World {

    pub fn draw(&self, pixels: &mut Pixels) {
        let frame_len = pixels.get_frame().len() as u32;
        let w_view = self.im_view.width();
        let h_view = self.im_view.height();
        if frame_len != w_view * h_view * 4 {
            pixels.resize_buffer(w_view, h_view);
        }
        let frame = pixels.get_frame();

        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let rgba = pixels_rgba_at(i, &self.im_view);
            pixel.copy_from_slice(&rgba);
        }
    }
    pub fn new(im_orig: ImageType) -> Self {
        Self {
            im_orig: im_orig.clone(),
            im_view: im_orig,
        }
    }
    pub fn update(
        mut self,
        input_event: &WinitInputHelper,
        shape_win: Shape,
        tools: &mut Vec<ToolWrapper>,
        pixels: &mut Pixels,
    ) -> Self {
        let mouse_pos = mouse_pos_transform(pixels, input_event.mouse());
        for tool in tools {
            let old_shape = shape_from_im(self.im_view());
            let mut transform = apply_tool_method!(
                tool,
                events_transform,
                input_event,
                shape_win,
                mouse_pos
            );
            self = transform(self);
            let new_shape = shape_from_im(self.im_view());
            if old_shape != new_shape {
                pixels.resize_buffer(new_shape.w, new_shape.h);
            }
        }
        self
    }
    pub fn im_view(&self) -> &ImageType {
        &self.im_view
    }
    pub fn im_view_mut(&mut self) -> &mut ImageType {
        &mut self.im_view
    }
    pub fn im_orig(&self) -> &ImageType {
        &self.im_orig
    }
    pub fn shape_orig(&self) -> Shape {
        Shape {
            w: self.im_orig.width(),
            h: self.im_orig.height(),
        }
    }

    pub fn get_pixel_on_orig(
        &self,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
        tools: &[ToolWrapper],
    ) -> Option<(u32, u32, [u8; 3])> {
        let mut mp = mouse_pos;
        let mut res = None;
        for tool in tools {
            let pos_rgb =
                apply_tool_method!(tool, get_pixel_on_orig, self.im_orig(), mp, shape_win);
            if let Some(prgb) = pos_rgb {
                mp = Some((prgb.0 as usize, prgb.1 as usize));
                res = pos_rgb;
            }
        }
        res
    }
    pub fn scale_to_shape(&mut self, shape: Shape, tools: &[ToolWrapper]) -> Shape {
        let mut new_shape = shape;
        for tool in tools {
            let im_view_new = apply_tool_method!(tool, scale_to_shape, self, &new_shape);
            if let Some(ivn) = im_view_new {
                new_shape = Shape {
                    w: ivn.width(),
                    h: ivn.height(),
                };
                self.im_view = ivn;
            }
        }
        new_shape
    }
}
#[cfg(test)]
use image::Rgb;
#[test]
fn test_rgba() {
    let mut im_test = ImageType::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(0, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(0, 1, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(64, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(7, 11, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(11 * 64 + 7, &im_test), [23, 23, 23, 255]);
}
