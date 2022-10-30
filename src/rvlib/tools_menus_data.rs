use tinyvec::{tiny_vec, TinyVec};

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
macro_rules! tools_menu_data_accessor_mut {
    ($actor:expr, $error_msg:expr) => {
        fn get_menu_data_mut<'a>(world: &'a mut World) -> &'a mut ToolsMenuData {
            world.data.menu_data.get_mut($actor).expect($error_msg)
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
pub const N_LABELS_ON_STACK: usize = 24;
type LabelsVec = TinyVec<[String; N_LABELS_ON_STACK]>;
#[derive(Clone, Debug, PartialEq)]
pub struct BboxSpecifics {
    pub new_label: String,
    labels: LabelsVec,
    pub idx_current: usize,
}
impl BboxSpecifics {
    pub fn remove(&mut self, idx: usize) {
        if self.labels.len() > 1 {
            self.labels.remove(idx);
            if self.idx_current >= idx {
                self.idx_current -= 1;
            }
        }
    }
    pub fn push(&mut self, elt: String) {
        self.labels.push(elt);
    }
    pub fn labels(&self) -> &LabelsVec {
        &self.labels
    }
}
impl Default for BboxSpecifics {
    fn default() -> Self {
        let new_label = "".to_string();
        let labels = tiny_vec!([String; N_LABELS_ON_STACK] => new_label.clone());
        BboxSpecifics {
            new_label,
            labels,
            idx_current: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ToolSpecifics {
    Bbox(BboxSpecifics),
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
    variant_access!(Bbox, bbox, &BboxSpecifics);
}
