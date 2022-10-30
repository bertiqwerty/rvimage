#[macro_export]
macro_rules! tools_menu_data_initializer {
    ($actor:expr, $variant:ident, $annotation_type:ident) => {
        fn initialize_tools_menu_data(mut world: World) -> World {
            if world.data.menu_data.get_mut($actor).is_none() {
                world.data.menu_data.insert(
                    $actor,
                    ToolsMenuData::new(ToolSpecifics::$variant($annotation_type::default())),
                );
            }
            world
        }
    };
}

#[macro_export]
macro_rules! tools_menu_data_accessor {
    ($actor:expr, $error_msg:expr) => {
        fn get_menu_data<'a>(world: &'a World) -> &'a ToolsMenuData {
            world.data.menu_data.get($actor).expect($error_msg)
        }
    };
}
macro_rules! variant_access {
    ($variant:ident, $func_name:ident, $return_type:ty) => {
        pub fn $func_name(self: &ToolsMenuData) -> $return_type {
            match &self.specifics {
                ToolSpecifics::$variant(x) => x,
            }
        }
    };
}
#[derive(Clone, Debug, PartialEq)]
pub enum ToolSpecifics {
    Bbox(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolsMenuData {
    pub specifics: ToolSpecifics,
    pub menu_active: bool,
}
impl ToolsMenuData {
    pub fn new(specifics: ToolSpecifics) -> Self {
        ToolsMenuData {
            specifics,
            menu_active: false,
        }
    }
    variant_access!(Bbox, bbox, &String);
}
