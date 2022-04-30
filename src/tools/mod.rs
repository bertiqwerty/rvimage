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
                 vec![$(ToolWrapper::$tool($tool::new())),+]
         }
    };
}
make_tools!(Rot90, Zoom);

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
macro_rules! make_tool_transform {
    ($self:expr, $w:expr, $shape_win:expr, $mouse_pos:expr, $event:expr, [$($mouse_event:ident),*], [$($key_event:expr),*]) => {
        if $event.image_loaded {
            $self.image_loaded($shape_win, $mouse_pos, $w)
        }
        else if $event.window_resized {
            $self.window_resized($shape_win, $mouse_pos, $w)
        }
        $(else if $event.input.$mouse_event(LEFT_BTN) {
            $self.$mouse_event(LEFT_BTN, $shape_win, $mouse_pos, $w)
        } else if $event.input.$mouse_event(RIGHT_BTN) {
            $self.$mouse_event(LEFT_BTN, $shape_win, $mouse_pos, $w)
        })*
        $(else if $event.input.key_pressed($key_event) {
            $self.key_pressed($key_event, $shape_win, $mouse_pos, $w)
        })*
        else {
            $w
        }
    };
}
