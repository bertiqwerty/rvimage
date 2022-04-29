mod core;
mod rot90;
mod zoom;
pub use self::core::{Tool, ToolTf, ViewCoordinateTf};
pub use rot90::Rot90;
use std::fmt::Debug;
pub use zoom::Zoom;

macro_rules! make_tools {
($($tool:ident),+) => {
        #[derive(Clone, Debug)]
        pub enum ToolWrapper {
            $($tool($tool)),+
        }
         pub fn make_tool_vec() -> Vec<ToolWrapper> {
                 vec![$(ToolWrapper::$tool($tool::new())),+]
         }
    };
}
make_tools!(Zoom, Rot90);

#[macro_export]
macro_rules! apply_tool_method {
    ($tool:expr, $f:ident, $($args:expr),*) => {
        match $tool {
            ToolWrapper::Rot90(z) => z.$f($($args,)*),
            ToolWrapper::Zoom(z) => z.$f($($args,)*)
        }
    };
}

#[macro_export]
macro_rules! make_event_handler_if_elses {
    ($self:expr, $event:expr, $mouse_pos:expr, [$($mouse_event:ident),*], [$($key_event:expr),*]) => {
        if $event.image_loaded {
            Box::new(move |w: World, shape_win: Shape| $self.image_loaded(shape_win, $mouse_pos, w))
        }
        else if $event.window_resized {
            Box::new(move |w: World, shape_win: Shape| $self.window_resized(shape_win, $mouse_pos, w))
        }
        $(else if $event.input.$mouse_event(LEFT_BTN) {
            Box::new(move |w: World, shape_win: Shape| $self.$mouse_event(LEFT_BTN, shape_win, $mouse_pos, w))
        } else if $event.input.$mouse_event(RIGHT_BTN) {
            Box::new(move |w: World, shape_win: Shape| $self.$mouse_event(LEFT_BTN, shape_win, $mouse_pos, w))
        })*
        $(else if $event.input.key_pressed($key_event) {
            Box::new(move |w: World, shape_win: Shape| $self.key_pressed($key_event, shape_win, $mouse_pos, w))
        })*
        else {
            Box::new(move |w: World, _: Shape| w)
        }
    };
}
