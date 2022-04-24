use image::{ImageBuffer, Rgb, imageops};
use winit_input_helper::WinitInputHelper;

use crate::{util::Shape, world::World};

use super::Tool;

pub struct Rot90 {
    n_rots: u8,
}


/// rotate 90 degrees counter clockwise
fn rot90(im: &ImageBuffer<Rgb<u8>, Vec<u8>>) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    imageops::rotate270(im)
}

impl Tool for Rot90 {
    fn new() -> Self {
        Self { n_rots: 0 }
    }

    /// what should happen to the state of this tool when a new image is loaded
    fn old_to_new(self) -> Self {
        Self::new()
    }

    fn events_transform(
        &mut self,
        input_event: &WinitInputHelper,
        window_shape: Shape,
        mouse_pos: Option<(usize, usize)>,
        world: &mut World,
    ) -> Option<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        None
    }

    fn scale_to_shape(&self, world: &mut World, shape: &Shape) -> Option<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        None
    }

    fn get_pixel_on_orig(
        &self,
        im_orig: &ImageBuffer<Rgb<u8>, Vec<u8>>,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
    ) -> Option<(u32, u32, [u8; 3])> {
        None
    }
}

#[test]
fn test_rot90_new() {
    let rot_tool = Rot90::new();
    assert_eq!(rot_tool.n_rots, 0);
}
#[test]
fn test_rotate() {
    let mut im = ImageBuffer::<Rgb<u8>, Vec<u8>>::new(16, 8);
    im.put_pixel(1, 1, Rgb([2u8, 2u8, 2u8]));
    let im_rotated = rot90(&im);
    assert_eq!((im_rotated.width(), im_rotated.height()), (8, 16));
    assert_eq!(im_rotated.get_pixel(1, 14).0, [2u8, 2u8, 2u8]);
}
