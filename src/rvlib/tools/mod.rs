mod bbox;
mod brush;
mod core;
mod rot90;
mod zoom;

use crate::{domain::Shape, history::History, world::World};

pub use self::core::Manipulate;
pub use bbox::BBox;
pub use brush::Brush;
pub use rot90::Rot90;
use std::fmt::Debug;
pub use zoom::Zoom;

pub const BBOX_NAME: &str = bbox::ACTOR_NAME;
pub const BRUSH_NAME: &str = "Brush";
pub const ZOOM_NAME: &str = "Zoom";
pub const ROT90_NAME: &str = "Rot90";

macro_rules! make_tools {
($(($tool:ident, $label:expr, $name:expr)),+) => {
        #[derive(Clone, Debug)]
        pub enum ToolWrapper {
            $($tool($tool)),+
        }
        pub fn make_tool_vec() -> Vec<ToolState> {
            vec![$(
                ToolState {
                    tool_wrapper: ToolWrapper::$tool($tool::new()),
                    is_active: false,
                    name: $name,
                    button_label: $label
                }),+]
        }
    };
}
make_tools!(
    (Rot90, "🔄", ROT90_NAME),
    (Brush, "✏", BRUSH_NAME),
    (BBox, "⬜", BBOX_NAME),
    (Zoom, "🔍", ZOOM_NAME)
);

#[macro_export]
macro_rules! apply_tool_method_mut {
    ($tool_state:expr, $f:ident, $($args:expr),*) => {
        match &mut $tool_state.tool_wrapper {
            ToolWrapper::Rot90(z) => z.$f($($args,)*),
            ToolWrapper::Brush(z) => z.$f($($args,)*),
            ToolWrapper::BBox(z) => z.$f($($args,)*),
            ToolWrapper::Zoom(z) => z.$f($($args,)*),
        }
    };
}

pub struct ToolState {
    pub tool_wrapper: ToolWrapper,
    is_active: bool,
    pub name: &'static str,
    pub button_label: &'static str,
}
impl ToolState {
    pub fn activate(
        &mut self,
        mut world: World,
        mut history: History,
        shape_win: Shape,
    ) -> (World, History) {
        self.is_active = true;
        (world, history) = apply_tool_method_mut!(self, on_activate, world, history, shape_win);
        (world, history)
    }
    pub fn deactivate(
        &mut self,
        mut world: World,
        mut history: History,
        shape_win: Shape,
    ) -> (World, History) {
        if self.is_active {
            (world, history) =
                apply_tool_method_mut!(self, on_deactivate, world, history, shape_win);
        }
        self.is_active = false;
        (world, history)
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
}
