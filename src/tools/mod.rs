mod core;
mod zoom;
mod rot90;
use std::fmt::Debug;
pub use self::core::Tool;
pub use zoom::Zoom;
pub use rot90::Rot90;

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

