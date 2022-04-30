use crate::{
    util::{Event, Shape},
    world::World,
};

pub trait Tool {
    fn new() -> Self
    where
        Self: Sized;

    fn coord_tf(
        &self,
        _world: &World,
        _shape_win: Shape,
        _mouse_pos: Option<(u32, u32)>
    ) -> Option<(u32, u32)> {
        None
    }

    fn events_tf<'a>(
        &'a mut self,
        world: World,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &Event,
    ) -> World;

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
