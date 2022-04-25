use winit_input_helper::WinitInputHelper;

use crate::{util::Shape, world::World, ImageType};

pub trait Tool {

    fn new() -> Self
    where
        Self: Sized;

    /// what should happen to the state of this tool when a new image is loaded
    fn old_to_new(self) -> Self;

    fn events_transform(
        &mut self,
        input_event: &WinitInputHelper,
        window_shape: Shape,
        mouse_pos_on_pixels: Option<(usize, usize)>,
        world: World,
    ) -> World;

    fn scale_to_shape(
        &self,
        world: &mut World,
        shape: &Shape,
    ) -> Option<ImageType>;

    fn get_pixel_on_orig(
        &self,
        im_orig: &ImageType,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
    ) -> Option<(u32, u32, [u8; 3])>;
}
