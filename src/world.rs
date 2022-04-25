use crate::tools::{Tool, ToolWrapper};
use crate::util::{mouse_pos_transform, Shape};
use crate::{apply_tool_method, apply_tool_method_mut};
use image::{ImageBuffer, Rgb};
use pixels::Pixels;
use winit_input_helper::WinitInputHelper;

/// Draw the image to the frame buffer.
///
/// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
fn pixels_rgba_at(i: usize, im_view: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> [u8; 4] {
    let x = (i % im_view.width() as usize) as u32;
    let y = (i / im_view.width() as usize) as u32;
    let rgb = im_view.get_pixel(x, y).0;
    let rgb_changed = rgb;
    [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff]
}
/// Everything we need to draw
pub struct World {
    im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>,
    im_view: ImageBuffer<Rgb<u8>, Vec<u8>>,
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
    pub fn new(im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>) -> Self {
        Self {
            im_orig: im_orig.clone(),
            im_view: im_orig,
        }
    }
    pub fn update(
        &mut self,
        input_event: &WinitInputHelper,
        shape_win: Shape,
        tools: &mut Vec<ToolWrapper>,
        pixels: &mut Pixels,
    ) {
        let mouse_pos = mouse_pos_transform(pixels, input_event.mouse());
        for tool in tools {
            let im_view_new = apply_tool_method_mut!(
                tool,
                events_transform,
                input_event,
                shape_win,
                mouse_pos,
                self
            );
            if let Some(ivn) = im_view_new {
                pixels.resize_buffer(ivn.width(), ivn.height());
                self.im_view = ivn;
            }
        }
    }
    pub fn im_view(&self) -> &ImageBuffer<Rgb<u8>, Vec<u8>> {
        &self.im_view
    }
    pub fn im_view_mut(&mut self) -> &mut ImageBuffer<Rgb<u8>, Vec<u8>> {
        &mut self.im_view
    }
    pub fn im_orig(&self) -> &ImageBuffer<Rgb<u8>, Vec<u8>> {
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

#[test]
fn test_rgba() {
    let mut im_test = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(64, 64);
    im_test.put_pixel(0, 0, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(0, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(0, 1, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(64, &im_test), [23, 23, 23, 255]);
    im_test.put_pixel(7, 11, Rgb([23, 23, 23]));
    assert_eq!(pixels_rgba_at(11 * 64 + 7, &im_test), [23, 23, 23, 255]);
}
