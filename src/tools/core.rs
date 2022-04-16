use pixels::Pixels;
use winit_input_helper::WinitInputHelper;

use crate::{util::Shape, world::World};

pub trait Tool {
    fn draw(&self, world: &World, pixels: &mut Pixels);

    fn new() -> Self
    where
        Self: Sized;
    fn old_to_new(self) -> Self;
    fn events_transform(
        &mut self,
        input_event: &WinitInputHelper,
        window_shape: Shape,
        pixels: &mut Pixels,
        world: &mut World,
    );
    fn scale_to_shape(&self, world: &mut World, shape: &Shape) -> Option<Shape>;

    fn get_pixel_on_orig(
        &self,
        world: &World,
        mouse_pos: Option<(usize, usize)>,
        shape_win: Shape,
    ) -> Option<(u32, u32, [u8; 3])>;
}
