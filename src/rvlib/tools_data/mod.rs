use crate::{
    domain::{Shape, BB},
    types::ViewImage,
};

pub use self::{
    bbox_data::BboxExportData, bbox_data::BboxSpecificData, brush_data::BrushToolData,
    coco_io::write_coco,
};
pub mod annotations;
pub mod bbox_data;
pub mod brush_data;
pub mod coco_io;
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
        pub(super) fn get_annos(world: &World) -> Option<&$annotations_type> {
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
            let shape = world.data.shape();
            world
                .data
                .tools_data_map
                .get_mut($actor)
                .expect($error_msg)
                .specifics
                .$access_func()
                .get_annos_mut(&current_file_path, shape)
        }
    };
}
#[macro_export]
macro_rules! tools_data_accessor_mut {
    ($actor:expr, $error_msg:expr) => {
        pub(super) fn get_tools_data_mut(world: &mut World) -> &mut ToolsData {
            world.data.tools_data_map.get_mut($actor).expect($error_msg)
        }
    };
}
#[macro_export]
macro_rules! tools_data_accessor {
    ($actor:expr, $error_msg:expr) => {
        pub(super) fn get_tools_data(world: &World) -> &ToolsData {
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
    Bbox(BboxSpecificData),
    Brush(BrushToolData),
}
impl ToolSpecifics {
    variant_access!(Bbox, bbox, &Self, &BboxSpecificData);
    variant_access!(Brush, brush, &Self, &BrushToolData);
    variant_access!(Bbox, bbox_mut, &mut Self, &mut BboxSpecificData);
    variant_access!(Brush, brush_mut, &mut Self, &mut BrushToolData);

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
                if let Some(annos) = bb_data.get_annos(file_path) {
                    im_view = annos.draw_on_view(
                        im_view,
                        zoom_box,
                        shape_orig,
                        shape_win,
                        bb_data.labels(),
                        bb_data.colors(),
                    );
                }
            }
            ToolSpecifics::Brush(brush_data) => {
                if let Some(annos) = brush_data.get_annos(file_path) {
                    im_view = annos.draw_on_view(im_view, zoom_box, shape_orig, shape_win);
                }
            }
        }
        im_view
    }
}
impl Default for ToolSpecifics {
    fn default() -> Self {
        ToolSpecifics::Bbox(BboxSpecificData::default())
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
