mod always_active_zoom;
mod bbox;
mod brush;
mod core;
mod rot90;
mod zoom;

use crate::{domain::Shape, history::History, world::World};

pub use self::core::Manipulate;
pub use always_active_zoom::AlwaysActiveZoom;
pub use bbox::BBox;
pub use brush::Brush;
pub use rot90::Rot90;
use std::fmt::Debug;
pub use zoom::Zoom;

pub const BBOX_NAME: &str = bbox::ACTOR_NAME;
pub const BRUSH_NAME: &str = "Brush";
pub const ZOOM_NAME: &str = "Zoom";
pub const ROT90_NAME: &str = "Rot90";
pub const ALWAYS_ACTIVE_ZOOM: &str = "AlwaysActiveZoom";

macro_rules! make_tools {
($(($tool:ident, $label:expr, $name:expr, $active:expr, $always_active:expr)),+) => {
        #[derive(Clone, Debug)]
        pub enum ToolWrapper {
            $($tool($tool)),+
        }
        pub fn make_tool_vec() -> Vec<ToolState> {
            vec![$(
                ToolState {
                    tool_wrapper: ToolWrapper::$tool($tool::new()),
                    is_active: $active,
                    is_always_active: $always_active,
                    name: $name,
                    button_label: $label
                }),+]
        }
    };
}
make_tools!(
    (Rot90, "🔄", ROT90_NAME, false, false),
    (Brush, "✏", BRUSH_NAME, false, false),
    (BBox, "⬜", BBOX_NAME, false, false),
    (Zoom, "🔍", ZOOM_NAME, false, false),
    (AlwaysActiveZoom, "AA🔍", ALWAYS_ACTIVE_ZOOM, true, true)
);

#[macro_export]
macro_rules! apply_tool_method_mut {
    ($tool_state:expr, $f:ident, $($args:expr),*) => {
        match &mut $tool_state.tool_wrapper {
            ToolWrapper::Rot90(z) => z.$f($($args,)*),
            ToolWrapper::Brush(z) => z.$f($($args,)*),
            ToolWrapper::BBox(z) => z.$f($($args,)*),
            ToolWrapper::Zoom(z) => z.$f($($args,)*),
            ToolWrapper::AlwaysActiveZoom(z) => z.$f($($args,)*),
        }
    };
}

pub struct ToolState {
    pub tool_wrapper: ToolWrapper,
    is_active: bool,
    is_always_active: bool, // no entry in the menu
    pub name: &'static str,
    pub button_label: &'static str,
}
impl ToolState {
    pub fn activate(&mut self, mut world: World, mut history: History) -> (World, History) {
        self.is_active = true;
        (world, history) = apply_tool_method_mut!(self, on_activate, world, history);
        (world, history)
    }
    pub fn deactivate(&mut self, mut world: World, mut history: History) -> (World, History) {
        if self.is_active {
            (world, history) = apply_tool_method_mut!(self, on_deactivate, world, history);
        }
        if !self.is_always_active {
            self.is_active = false;
        }
        (world, history)
    }
    pub fn is_active(&self) -> bool {
        self.is_active
    }
    pub fn is_always_active(&self) -> bool {
        self.is_always_active
    }
}
