use crate::{
    types::ViewImage,
    util::{Shape, BB},
};

pub use self::{bbox_data::BboxSpecifics, brush_data::BrushSpecifics};
pub mod bbox_data;
pub mod brush_data;
#[macro_export]
macro_rules! tools_data_initializer {
    ($actor:expr, $variant:ident, $tool_data_type:ident) => {
        pub(super) fn initialize_tools_menu_data(mut world: World) -> World {
            if world.data.tools_data_map.get_mut($actor).is_none() {
                world.data.tools_data_map.insert(
                    $actor,
                    ToolsData::new(ToolSpecifics::$variant($tool_data_type::default())),
                );
            }
            world
        }
    };
}

#[macro_export]
macro_rules! annotations_accessor {
    ($actor:expr, $access_func:ident, $error_msg:expr, $annotations_type:ty) => {
        pub(super) fn get_annos(world: &World) -> &$annotations_type {
            let current_file_path = world.data.meta_data.file_path.as_ref().unwrap();
            world
                .data
                .tools_data_map
                .get($actor)
                .expect($error_msg)
                .specifics
                .$access_func()
                .get_annos(&current_file_path)
        }
    };
}
#[macro_export]
macro_rules! annotations_accessor_mut {
    ($actor:expr, $access_func:ident, $error_msg:expr, $annotations_type:ty) => {
        pub(super) fn get_annos_mut(world: &mut World) -> &mut $annotations_type {
            let current_file_path = world.data.meta_data.file_path.as_ref().unwrap();
            world
                .data
                .tools_data_map
                .get_mut($actor)
                .expect($error_msg)
                .specifics
                .$access_func()
                .get_annos_mut(&current_file_path)
        }
    };
}
#[macro_export]
macro_rules! tools_data_accessor_mut {
    ($actor:expr, $error_msg:expr) => {
        pub(super) fn get_tools_data_mut<'a>(world: &'a mut World) -> &'a mut ToolsData {
            world.data.tools_data_map.get_mut($actor).expect($error_msg)
        }
    };
}
#[macro_export]
macro_rules! tools_data_accessor {
    ($actor:expr, $error_msg:expr) => {
        pub(super) fn get_tools_data<'a>(world: &'a World) -> &'a ToolsData {
            world.data.tools_data_map.get($actor).expect($error_msg)
        }
    };
}

macro_rules! variant_access {
    ($variant:ident, $func_name:ident, $self:ty, $return_type:ty) => {
        pub fn $func_name(self: $self) -> $return_type {
            match self {
                ToolSpecifics::$variant(x) => x,
                _ => panic!("this is not a {}", stringify!($variant)),
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ToolSpecifics {
    Bbox(BboxSpecifics),
    Brush(BrushSpecifics),
}
impl ToolSpecifics {
    variant_access!(Bbox, bbox, &Self, &BboxSpecifics);
    variant_access!(Brush, brush, &Self, &BrushSpecifics);
    variant_access!(Bbox, bbox_mut, &mut Self, &mut BboxSpecifics);
    variant_access!(Brush, brush_mut, &mut Self, &mut BrushSpecifics);

    pub fn draw_on_view(
        &self,
        mut im_view: ViewImage,
        zoom_box: &Option<BB>,
        shape_orig: Shape,
        shape_win: Shape,
        file_path: &str,
    ) -> ViewImage {
        match &self {
            ToolSpecifics::Bbox(bb_data) => {
                im_view = bb_data.get_annos(file_path).draw_on_view(
                    im_view,
                    zoom_box,
                    shape_orig,
                    shape_win,
                    bb_data.labels(),
                    bb_data.colors(),
                );
            }
            ToolSpecifics::Brush(brush_data) => {
                im_view = brush_data
                    .get_annos(file_path)
                    .draw_on_view(im_view, zoom_box, shape_orig, shape_win);
            }
        }
        im_view
    }
}
impl Default for ToolSpecifics {
    fn default() -> Self {
        ToolSpecifics::Bbox(BboxSpecifics::default())
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ToolsData {
    pub specifics: ToolSpecifics,
    pub menu_active: bool,
}
impl ToolsData {
    pub fn new(specifics: ToolSpecifics) -> Self {
        ToolsData {
            specifics,
            menu_active: false,
        }
    }
}
