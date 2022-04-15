use crate::tools::{make_tool_vec, Tool, ToolWrapper};
use crate::util::Shape;
use crate::{apply_tool_method, apply_tool_method_mut, map_tool_method};
use image::{ImageBuffer, Rgb};
use pixels::Pixels;
use winit::dpi::PhysicalSize;
use winit::window::Window;
use winit_input_helper::WinitInputHelper;

/// Everything we need to draw
pub struct World {
    im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>,
    im_view: ImageBuffer<Rgb<u8>, Vec<u8>>,
    tools: Vec<ToolWrapper>,
}

impl World {
    pub fn new(im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>, old_world: Option<&World>) -> Self {
        let tools = match old_world {
            None => make_tool_vec(),
            Some(ow) => ow
                .tools
                .iter()
                .map(|tool| map_tool_method!(tool, old_to_new,))
                .collect::<Vec<ToolWrapper>>(),
        };
        Self {
            im_orig: im_orig.clone(),
            im_view: im_orig,
            tools,
        }
    }
    pub fn update(
        &mut self,
        input_events: &WinitInputHelper,
        window: &Window,
        pixels: &mut Pixels,
    ) {
        for tool in self.tools.clone() {
            apply_tool_method_mut!(tool, events_transform, input_events, window, pixels, self);
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
        size_win: &PhysicalSize<u32>,
    ) -> Option<(u32, u32, [u8; 3])> {
        let mut mp = mouse_pos;
        let mut res = None;
        for tool in self.tools.clone() {
            let pos_rgb = apply_tool_method!(tool, get_pixel_on_orig, self, mp, size_win);
            if let Some(prgb) = pos_rgb {
                mp = Some((prgb.0 as usize, prgb.1 as usize));
                res = pos_rgb;
            }
        }
        res
    }
    pub fn scale_to_shape(&mut self, shape: Shape) -> Shape {
        let mut new_shape = shape;
        for tool in self.tools.clone() {
            let tmp_shape = apply_tool_method!(tool, scale_to_shape, self, &new_shape);
            if let Some(ts) = tmp_shape {
                new_shape = ts;
            }
        }
        new_shape
    }
    pub fn draw(&self, pixels: &mut Pixels) {
        for tool in self.tools.clone() {
            apply_tool_method!(tool, draw, self, pixels);
        }
    }
}

// #[test]
// fn test_world() {
//     {
//         // some general basic tests
//         let (w, h) = (100, 100);
//         let size_win = PhysicalSize::<u32>::new(w, h);
//         let mut im = ImageBuffer::<Rgb<u8>, _>::new(w, h);
//         im[(10, 10)] = Rgb::<u8>::from([4, 4, 4]);
//         im[(20, 30)] = Rgb::<u8>::from([5, 5, 5]);
//         let mut world = World::new(im, None);
//         assert_eq!((w, h), shape_unscaled(&world.zoom, world.shape_orig()));
//         world.zoom = make_zoom((10, 10), (60, 60), (w, h), &size_win, &None);
//         let zoom = world.zoom.unwrap();
//         assert_eq!(Some((50, 50)), Some((zoom.w, zoom.h)));
//         assert_eq!(
//             Some((10, 10, [4, 4, 4])),
//             world.get_pixel_on_orig(Some((0, 0)), &size_win)
//         );
//         assert_eq!(
//             Some((20, 30, [5, 5, 5])),
//             world.get_pixel_on_orig(Some((20, 40)), &size_win)
//         );
//         assert_eq!((100, 100), (world.im_view.width(), world.im_view.height()));
//     }
//     {
//         // another test on finding pixels in the original image
//         let (win_w, win_h) = (200, 100);
//         let size_win = PhysicalSize::<u32>::new(win_w, win_h);
//         let (w_im_o, h_im_o) = (100, 50);
//         let im = ImageBuffer::<Rgb<u8>, _>::new(w_im_o, h_im_o);
//         let mut world = World::new(im, None);
//         world.zoom = make_zoom((10, 20), (50, 40), (w_im_o, h_im_o), &size_win, &None);
//         let zoom = world.zoom.unwrap();
//         assert_eq!(Some((20, 10)), Some((zoom.w, zoom.h)));
//     }
// }
