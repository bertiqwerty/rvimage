use image::imageops;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::{util::Shape, world::World, ImageType, make_event_handler_if_elses};

use super::Tool;

pub struct Rot90 {
    n_rots: u8,
}

impl Rot90 {
    
    fn key_pressed(
        &mut self,
        key: VirtualKeyCode,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
    ) -> World {
        if key == VirtualKeyCode::R {
            
            *world.im_view_mut() = rot90(world.im_view());
        }
        world
    }
}

/// rotate 90 degrees counter clockwise
fn rot90(im: &ImageType) -> ImageType {
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

    fn events_transform<'a>(
        &'a mut self,
        input_event: &'a WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
    ) -> Box<dyn 'a + FnMut(World) -> World> {
        make_event_handler_if_elses!(
            self,
            input_event,
            shape_win,
            mouse_pos,
            [],
            [VirtualKeyCode::R]
        )
    }

    fn scale_to_shape(&self, world: &mut World, shape: &Shape) -> Option<ImageType> {
        None
    }

    fn get_pixel_on_orig(
        &self,
        im_orig: &ImageType,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
    ) -> Option<(u32, u32, [u8; 3])> {
        None
    }
}
#[cfg(test)]
use image::Rgb;
#[test]
fn test_rot90_new() {
    let rot_tool = Rot90::new();
    assert_eq!(rot_tool.n_rots, 0);
}
#[test]
fn test_rotate() {
    let mut im = ImageType::new(16, 8);
    im.put_pixel(1, 1, Rgb([2u8, 2u8, 2u8]));
    let im_rotated = rot90(&im);
    assert_eq!((im_rotated.width(), im_rotated.height()), (8, 16));
    assert_eq!(im_rotated.get_pixel(1, 14).0, [2u8, 2u8, 2u8]);
}
