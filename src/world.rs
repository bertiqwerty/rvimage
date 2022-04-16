use crate::{apply_tool_method_mut, apply_tool_method};
use crate::tools::{Tool, ToolWrapper};
use crate::util::Shape;
use image::{ImageBuffer, Rgb};
use pixels::Pixels;
use winit_input_helper::WinitInputHelper;

/// Everything we need to draw
pub struct World {
    im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>,
    im_view: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

impl World {
    pub fn new(im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>) -> Self {
        
        Self {
            im_orig: im_orig.clone(),
            im_view: im_orig,
        }
    }
    pub fn update(
        &mut self,
        input_events: &WinitInputHelper,
        shape_win: Shape,
        tools: &mut Vec<ToolWrapper>,
        pixels: &mut Pixels,
    ) {
        
        for tool in tools {
            apply_tool_method_mut!(tool, events_transform, input_events, shape_win, pixels, self);
            // apply_tool_mut(tool, |t| t.events_transform(input_events, window, pixels, self));
        }
    }
    pub fn im_view(&self) -> &ImageBuffer<Rgb<u8>, Vec<u8>> {
        &self.im_view
    }
    pub fn set_im_view(&mut self, im_view: ImageBuffer<Rgb<u8>, Vec<u8>>) {
        self.im_view = im_view;
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
        tools: &Vec<ToolWrapper>,
    ) -> Option<(u32, u32, [u8; 3])> {
        let mut mp = mouse_pos;
        let mut res = None;
        for tool in tools {
            let pos_rgb = apply_tool_method!(tool, get_pixel_on_orig, self, mp, shape_win);
            if let Some(prgb) = pos_rgb {
                mp = Some((prgb.0 as usize, prgb.1 as usize));
                res = pos_rgb;
            }
        }
        res
    }
    pub fn scale_to_shape(&mut self, shape: Shape, tools: &Vec<ToolWrapper>) -> Shape {
        let mut new_shape = shape;
        for tool in tools {
            let tmp_shape = apply_tool_method!(tool, scale_to_shape, self, &new_shape);
            if let Some(ts) = tmp_shape {
                new_shape = ts;
            }
        }
        new_shape
    }
    pub fn draw(&self, pixels: &mut Pixels, tools: &Vec<ToolWrapper>) {
        for tool in tools {
            apply_tool_method!(tool, draw,self, pixels);
        }
    }
}

