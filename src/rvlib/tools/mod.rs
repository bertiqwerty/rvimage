mod bbox;
mod brush;
mod core;
mod rot90;
mod zoom;

use crate::{history::History, util::Shape, world::World};

pub use self::core::{Manipulate, MetaData};
pub use bbox::BBox;
pub use brush::Brush;
pub use rot90::Rot90;
use std::fmt::Debug;
pub use zoom::Zoom;

macro_rules! make_tools {
($(($tool:ident, $label:expr)),+) => {
        #[derive(Clone, Debug)]
        pub enum ToolWrapper {
            $($tool($tool)),+
        }
        pub fn make_tool_vec() -> Vec<ToolState> {
            vec![$(
                ToolState {
                    tool: ToolWrapper::$tool($tool::new()),
                    is_active: false,
                    name: stringify!($tool),
                    button_label: $label
                }),+]
        }
    };
}
make_tools!((Rot90, "🔄"), (Zoom, "🔍"), (Brush, "✏"), (BBox, "⬜"));

#[macro_export]
macro_rules! apply_tool_method {
    ($tool:expr, $f:ident, $($args:expr),*) => {
        match &mut $tool.tool {
            ToolWrapper::Rot90(z) => z.$f($($args,)*),
            ToolWrapper::Zoom(z) => z.$f($($args,)*),
            ToolWrapper::Brush(z) => z.$f($($args,)*),
            ToolWrapper::BBox(z) => z.$f($($args,)*),
        }
    };
}

pub struct ToolState {
    pub tool: ToolWrapper,
    is_active: bool,
    pub name: &'static str,
    pub button_label: &'static str,
}
impl ToolState {
    pub fn activate(&mut self) {
        self.is_active = true;
    }
    pub fn deactivate(
        &mut self,
        mut world: World,
        mut history: History,
        shape_win: Shape,
        meta_data: &MetaData,
    ) -> (World, History) {
        if self.is_active {
            (world, history) =
                apply_tool_method!(self, on_deactivate, world, history, shape_win, meta_data);
        }
        self.is_active = false;
        (world, history)
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
}
