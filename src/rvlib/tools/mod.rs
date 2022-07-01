mod brush;
mod core;
mod rot90;
mod zoom;
pub use self::core::Manipulate;
pub use brush::Brush;
pub use rot90::Rot90;
use std::fmt::Debug;
pub use zoom::Zoom;

pub struct ToolState {
    pub tool: ToolWrapper,
    pub is_active: bool,
    pub name: String,
}

macro_rules! make_tools {
($($tool:ident),+) => {
        #[derive(Clone, Debug)]
        pub enum ToolWrapper {
            $($tool($tool)),+
        }
        pub fn make_tool_vec() -> Vec<ToolState> {
            vec![$(
                ToolState { 
                    tool: ToolWrapper::$tool($tool::new()), 
                    is_active: false, 
                    name: stringify!($tool).to_string()
                }),+]
        }
    };
}
make_tools!(Rot90, Zoom, Brush);

#[macro_export]
macro_rules! apply_tool_method {
    ($tool:expr, $f:ident, $($args:expr),*) => {
        match &mut $tool.tool {
            ToolWrapper::Rot90(z) => z.$f($($args,)*),
            ToolWrapper::Zoom(z) => z.$f($($args,)*),
            ToolWrapper::Brush(z) => z.$f($($args,)*)
        }
    };
}
