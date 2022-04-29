use crate::{
    util::{Event, Shape},
    world::World,
};

pub type ToolTf<'a> = Box<dyn 'a + FnMut(World, Shape) -> World>;
pub type ViewCoordinateTf<'a> = Box<dyn 'a + Fn(Option<(u32, u32)>, &World, Shape) -> Option<(u32, u32)>>;
pub trait Tool {
    fn new() -> Self
    where
        Self: Sized;

    fn events_transform<'a>(
        &'a mut self,
        input_event: &Event,
        mouse_pos_on_pixels: Option<(usize, usize)>,
    ) -> (ToolTf, Option<ViewCoordinateTf>);

    fn image_loaded(
        &mut self,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        world: World,
    ) -> World {
        world
    }
    fn window_resized(
        &mut self,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        world: World,
    ) -> World {
        world
    }
}
