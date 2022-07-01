use crate::{
    history::History,
    util::{Event, Shape},
    world::World,
};

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

    /// Transformation of the coordinates to show pixel positions and RGB values correctly.
    fn coord_tf(
        &self,
        _world: &World,
        _shape_win: Shape,
        _mouse_pos: Option<(u32, u32)>,
    ) -> Option<(u32, u32)> {
        None
    }

    /// All events that are used by a tool are implemented in here. Use the macro [`make_tool_transform`](make_tool_transform). See, e.g.,
    /// [`Zoom::events_tf`](crate::tools::Zoom::events_tf).
    fn events_tf<'a>(
        &'a mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &Event,
    ) -> (World, History);

    /// Special event that is triggered on load of a new image.
    fn image_loaded(
        &mut self,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        world: World,
    ) -> World {
        world
    }

    /// Special event that is triggered on window resize.
    fn window_resized(
        &mut self,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        world: World,
    ) -> World {
        world
    }
}

// applies the tool transformation to the world
#[macro_export]
macro_rules! make_tool_transform {
    ($self:expr, $world:expr, $history:expr, $shape_win:expr, $mouse_pos:expr, $event:expr, [$($mouse_event:ident),*], [$($key_event:expr),*]) => {
        if $event.image_loaded {
            ($self.image_loaded($shape_win, $mouse_pos, $world), $history)
        }
        else if $event.window_resized {
            ($self.window_resized($shape_win, $mouse_pos, $world), $history)
        }
        $(else if $event.input.$mouse_event(LEFT_BTN) {
            $self.$mouse_event(LEFT_BTN, $shape_win, $mouse_pos, $world, $history)
        } else if $event.input.$mouse_event(RIGHT_BTN) {
            $self.$mouse_event(RIGHT_BTN, $shape_win, $mouse_pos, $world, $history)
        })*
        $(else if $event.input.key_pressed($key_event) {
            $self.key_pressed($key_event, $shape_win, $mouse_pos, $world, $history)
        })*
        else {
            ($world, $history)
        }
    };
}
