mod annotate;
mod core;
mod rot90;
mod zoom;
pub use self::core::Tool;
pub use annotate::Annotate;
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
