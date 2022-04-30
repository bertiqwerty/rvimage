use image::imageops::{self, FilterType};
use winit::event::VirtualKeyCode;

use crate::{
    make_tool_transform,
    util::{shape_scaled, Event, Shape},
    world::World,
    ImageType,
};

use super::{Tool, ToolTf, core::ViewCoordinateTf};

/// rotate 90 degrees counter clockwise
fn rot90(im: &ImageType, shape_unscaled: Shape, shape_win: Shape) -> ImageType {
    let shape_scaled = shape_scaled(shape_unscaled, shape_win);
    imageops::resize(
        &imageops::rotate270(im),
        shape_scaled.w,
        shape_scaled.h,
        FilterType::Nearest,
    )
}
#[derive(Clone, Copy, Debug)]
pub struct Rot90;

impl Rot90 {
    fn key_pressed(
        &mut self,
        key: VirtualKeyCode,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
    ) -> World {
        if key == VirtualKeyCode::R {
            *world.im_orig_mut() = imageops::rotate270(world.im_orig());
            *world.im_view_mut() =
                rot90(world.im_view(), Shape::from_im(world.im_orig()), shape_win);
        }
        world
    }
}

impl Tool for Rot90 {
    fn new() -> Self {
        Self {}
    }

    fn events_transform<'a>(
        &'a mut self,
    ) -> (ToolTf, Option<ViewCoordinateTf>) {
        let tt: ToolTf = make_tool_transform!(self, [], [VirtualKeyCode::R]);
        (tt, None)
    }

}
// #[cfg(test)]
// use image::Rgb;
// #[test]
// fn test_rotate() {
//     let mut im = ImageType::new(16, 8);
//     im.put_pixel(1, 1, Rgb([2u8, 2u8, 2u8]));
//     let im_rotated = rot90(&im, Shape{});
//     assert_eq!((im_rotated.width(), im_rotated.height()), (8, 16));
//     assert_eq!(im_rotated.get_pixel(1, 14).0, [2u8, 2u8, 2u8]);
// }
