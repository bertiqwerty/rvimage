use winit_input_helper::WinitInputHelper;

use crate::{history::History, util::Shape, world::World};

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

    /// All events that are used by a tool are implemented in here. Use the macro [`make_tool_transform`](make_tool_transform). See, e.g.,
    /// [`Zoom::events_tf`](crate::tools::Zoom::events_tf).
    fn events_tf<'a>(
        &'a mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        input_event: &WinitInputHelper,
    ) -> (World, History);
}

// applies the tool transformation to the world
#[macro_export]
macro_rules! make_tool_transform {
    ($self:expr, $world:expr, $history:expr, $shape_win:expr, $mouse_pos:expr, $event:expr, [$($mouse_event:ident),*], [$($key_event:expr),*]) => {
        if false {
            ($world, $history)
        }
        $(else if $event.$mouse_event(LEFT_BTN) {
            $self.$mouse_event(LEFT_BTN, $shape_win, $mouse_pos, $world, $history)
        } else if $event.$mouse_event(RIGHT_BTN) {
            $self.$mouse_event(RIGHT_BTN, $shape_win, $mouse_pos, $world, $history)
        })*
        $(else if $event.key_pressed($key_event) {
            $self.key_pressed($key_event, $shape_win, $mouse_pos, $world, $history)
        })*
        else {
            ($world, $history)
        }
    };
}
