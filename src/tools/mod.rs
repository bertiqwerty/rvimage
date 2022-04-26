mod core;
mod rot90;
mod zoom;
pub use self::core::Tool;
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
                 vec![ToolWrapper::Zoom(Zoom::new())]
         }
    };
}
make_tools!(Zoom);

#[macro_export]
macro_rules! map_tool_method {
    ($tool:expr, $f:ident, $($args:expr),*) => {
        match $tool {
            ToolWrapper::Zoom(z) => ToolWrapper::Zoom(z.clone().$f($($args,)*))
        }
    };
}
#[macro_export]
macro_rules! apply_tool_method {
    ($tool:expr, $f:ident, $($args:expr),*) => {
        match $tool {
            ToolWrapper::Zoom(z) => z.$f($($args,)*)
        }
    };
}

#[macro_export]
macro_rules! make_event_handler_if_elses {
    ($self:expr, $input_event:expr, $shape_win:expr, $mouse_pos:expr, [$($mouse_event:ident),*], [$($key_event:expr),*]) => {
        Box::new(move |w: World|
        if false {
            w
        }
        $(else if $input_event.$mouse_event(LEFT_BTN) {
            $self.$mouse_event(LEFT_BTN, $shape_win, $mouse_pos, w)
        } else if $input_event.$mouse_event(RIGHT_BTN) {
            $self.$mouse_event(LEFT_BTN, $shape_win, $mouse_pos, w)
        })*
        $(else if $input_event.key_pressed($key_event) {
            $self.key_pressed($key_event, $shape_win, $mouse_pos, w)
        })*
        else {
            w
        })
    };
}
