use image::{
    imageops::{self, FilterType},
    GenericImageView, ImageBuffer, Rgb,
};
use pixels::Pixels;
use winit::dpi::PhysicalSize;

const MIN_CROP: u32 = 10;

/// shape of the image that fits into the window
fn shape_scaled(shape_unscaled: (u32, u32), shape_win: (u32, u32)) -> (u32, u32) {
    let (w_unscaled, h_unscaled) = shape_unscaled;
    let (w_win, h_win) = shape_win;
    let w_ratio = w_unscaled as f64 / w_win as f64;
    let h_ratio = h_unscaled as f64 / h_win as f64;
    let ratio = w_ratio.max(h_ratio);
    let w_new = (w_unscaled as f64 / ratio) as u32;
    let h_new = (h_unscaled as f64 / ratio) as u32;
    (w_new, h_new)
}

/// shape without scaling according to crop
fn shape_unscaled(crop: &Option<Crop>, shape_orig: (u32, u32)) -> (u32, u32) {
    crop.map_or(shape_orig, |c| (c.w, c.h))
}

fn make_crop(
    mouse_pos_start: (usize, usize),
    mouse_pos_end: (usize, usize),
    shape_orig: (u32, u32),
    size_win: &PhysicalSize<u32>,
    crop: &Option<Crop>,
) -> Option<Crop> {
    let prs_orig = mouse_pos_to_orig_pos(Some(mouse_pos_start), shape_orig, size_win, crop);
    let rel_orig = mouse_pos_to_orig_pos(Some(mouse_pos_end), shape_orig, size_win, crop);

    match (prs_orig, rel_orig) {
        (Some((px, py)), Some((rx, ry))) => {
            let x_min = px.min(rx) as u32;
            let y_min = py.min(ry) as u32;
            let x_max = px.max(rx) as u32;
            let y_max = py.max(ry) as u32;

            let w = x_max - x_min;
            let h = y_max - y_min;
            if w >= MIN_CROP && h >= MIN_CROP {
                Some(Crop {
                    x: x_min,
                    y: y_min,
                    w,
                    h,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Converts the mouse position to the coordinates of the original image
fn mouse_pos_to_orig_pos(
    mouse_pos: Option<(usize, usize)>,
    shape_orig: (u32, u32),
    size_win: &PhysicalSize<u32>,
    crop: &Option<Crop>,
) -> Option<(u32, u32)> {
    let (w_unscaled, h_unscaled) = shape_unscaled(crop, shape_orig);
    let (w_im_orig, h_im_orig) = shape_orig;
    let (w_scaled, h_scaled) =
        shape_scaled((w_unscaled, h_unscaled), (size_win.width, size_win.height));

    let (x_off, y_off) = match crop {
        Some(c) => (c.x, c.y),
        _ => (0, 0),
    };

    let coord_trans_2_orig = |x: u32, n_transformed: u32, n_orig: u32| -> u32 {
        (x as f64 / n_transformed as f64 * n_orig as f64) as u32
    };

    match mouse_pos {
        Some((x, y)) => {
            let x_orig = x_off + coord_trans_2_orig(x as u32, w_scaled, w_unscaled);
            let y_orig = y_off + coord_trans_2_orig(y as u32, h_scaled, h_unscaled);
            if x_orig < w_im_orig && y_orig < h_im_orig {
                Some((x_orig, y_orig))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn move_crop(
    m_press: (usize, usize),
    m_held: (usize, usize),
    shape_orig: (u32, u32),
    size_win: &PhysicalSize<u32>,
    crop: &Option<Crop>,
) -> Option<Crop> {
    let press_orig = mouse_pos_to_orig_pos(Some(m_press), shape_orig, size_win, crop);
    let held_orig = mouse_pos_to_orig_pos(Some(m_held), shape_orig, size_win, crop);
    let (w_im_orig, h_im_orig) = shape_orig;
    match (press_orig, held_orig, crop) {
        (Some((px, py)), Some((hx, hy)), Some(c)) => {
            let x_shift: i32 = px as i32 - hx as i32;
            let y_shift: i32 = py as i32 - hy as i32;
            let x_new = c.x as i32 + x_shift;
            let y_new = c.y as i32 + y_shift;
            if x_new >= 0
                && y_new >= 0
                && x_new as u32 + c.w < w_im_orig
                && y_new as u32 + c.h < h_im_orig
            {
                Some(Crop {
                    x: x_new as u32,
                    y: y_new as u32,
                    w: c.w,
                    h: c.h,
                })
            } else {
                *crop
            }
        }
        _ => *crop,
    }
}

/// Draw the image to the frame buffer.
///
/// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
pub fn draw(
    im_view: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    draw_crop: &Option<Crop>,
    pixels: &mut Pixels,
) {
    let frame_len = pixels.get_frame().len() as u32;
    if frame_len != im_view.width() * im_view.height() * 4 {
        pixels.resize_buffer(im_view.width(), im_view.height())
    }
    let frame = pixels.get_frame();

    for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
        let x = (i % im_view.width() as usize) as u32;
        let y = (i / im_view.width() as usize) as u32;
        let rgb = im_view.get_pixel(x, y).0;
        let rgb_changed = if let Some(dc) = draw_crop {
            let offset = 50;
            let change = |x| if 255 - x > offset { x + offset } else { 255 };
            if x >= dc.x && y >= dc.y && x < dc.x + dc.w && y < dc.y + dc.h {
                [change(rgb[0]), change(rgb[1]), change(rgb[2])]
            } else {
                rgb
            }
        } else {
            rgb
        };
        let rgba = [rgb_changed[0], rgb_changed[1], rgb_changed[2], 0xff];

        pixel.copy_from_slice(&rgba);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Everything we need to draw
pub struct World {
    im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>,
    im_view: ImageBuffer<Rgb<u8>, Vec<u8>>,
    crop: Option<Crop>,
    draw_crop: Option<Crop>, // for drawing a crop animation
}

impl World {
    pub fn new(im_orig: ImageBuffer<Rgb<u8>, Vec<u8>>) -> Self {
        Self {
            im_orig: im_orig.clone(),
            im_view: im_orig,
            crop: None,
            draw_crop: None,
        }
    }

    pub fn hide_draw_crop(&mut self) {
        self.draw_crop = None;
    }

    pub fn show_draw_crop(&mut self, crop: Crop) {
        self.draw_crop = Some(crop);
    }

    pub fn uncrop(&mut self) {
        self.crop = None;
    }

    pub fn apply_crop(&mut self, crop: &Option<Crop>) {
        self.crop = *crop;
    }

    pub fn crop(
        &mut self,
        mouse_pos_start: (usize, usize),
        mouse_pos_end: (usize, usize),
        size_win: &PhysicalSize<u32>,
    ) {
        self.crop = make_crop(
            mouse_pos_start,
            mouse_pos_end,
            self.shape_orig(),
            size_win,
            &self.crop,
        )
    }

    pub fn move_crop(
        &mut self,
        mouse_pos_start: (usize, usize),
        mouse_pos_end: (usize, usize),
        size_win: &PhysicalSize<u32>,
    ) {
        self.crop = move_crop(
            mouse_pos_start,
            mouse_pos_end,
            self.shape_orig(),
            size_win,
            &self.crop,
        )
    }

    pub fn get_crop(&self) -> Option<Crop> {
        self.crop
    }

    pub fn scale_to_match_win_inner(&mut self, w_win: u32, h_win: u32) -> (u32, u32) {
        let (w_unscaled, h_unscaled) = shape_unscaled(&self.crop, self.shape_orig());
        let (w_new, h_new) = shape_scaled((w_unscaled, h_unscaled), (w_win, h_win));

        match self.crop {
            Some(c) => {
                let cropped_view = self.im_orig.view(c.x, c.y, c.w, c.h);
                self.im_view = imageops::resize(&*cropped_view, w_new, h_new, FilterType::Nearest);
            }
            None => {
                self.im_view = imageops::resize(&self.im_orig, w_new, h_new, FilterType::Nearest);
            }
        }

        (w_new, h_new)
    }

    pub fn shape_orig(&self) -> (u32, u32) {
        (self.im_orig.width(), self.im_orig.height())
    }

    pub fn get_pixel_on_orig(
        &self,
        mouse_pos: Option<(usize, usize)>,
        size_win: &PhysicalSize<u32>,
    ) -> Option<(u32, u32, [u8; 3])> {
        let pos = mouse_pos_to_orig_pos(mouse_pos, self.shape_orig(), size_win, &self.crop);
        pos.map(|(x, y)| (x, y, self.im_orig.get_pixel(x, y).0))
    }

    pub fn draw(&self, pixels: &mut Pixels) {
        draw(&self.im_view, &self.draw_crop, pixels)
    }
}

#[test]
fn test_world() {
    {
        // some general basic tests
        let (w, h) = (100, 100);
        let size_win = PhysicalSize::<u32>::new(w, h);
        let mut im = ImageBuffer::<Rgb<u8>, _>::new(w, h);
        im[(10, 10)] = Rgb::<u8>::from([4, 4, 4]);
        im[(20, 30)] = Rgb::<u8>::from([5, 5, 5]);
        let mut world = World::new(im);
        assert_eq!((w, h), shape_unscaled(&world.crop, world.shape_orig()));
        world.crop = make_crop((10, 10), (60, 60), (w, h), &size_win, &None);
        let crop = world.crop.unwrap();
        assert_eq!(Some((50, 50)), Some((crop.w, crop.h)));
        assert_eq!(
            Some((10, 10, [4, 4, 4])),
            world.get_pixel_on_orig(Some((0, 0)), &size_win)
        );
        assert_eq!(
            Some((20, 30, [5, 5, 5])),
            world.get_pixel_on_orig(Some((20, 40)), &size_win)
        );
        assert_eq!((100, 100), (world.im_view.width(), world.im_view.height()));
    }
    {
        // another test on finding pixels in the original image
        let (win_w, win_h) = (200, 100);
        let size_win = PhysicalSize::<u32>::new(win_w, win_h);
        let (w_im_o, h_im_o) = (100, 50);
        let im = ImageBuffer::<Rgb<u8>, _>::new(w_im_o, h_im_o);
        let mut world = World::new(im);
        world.crop = make_crop((10, 20), (50, 40), (w_im_o, h_im_o), &size_win, &None);
        let crop = world.crop.unwrap();
        assert_eq!(Some((20, 10)), Some((crop.w, crop.h)));
    }
}
