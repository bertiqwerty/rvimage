pub use self::bbox_annotations::BboxAnnotations;
pub use self::brush_annotations::BrushAnnotations;
mod bbox_annotations;
mod brush_annotations;
#[macro_export]
macro_rules! implement_annotations_getters {
    ($default:expr, $tool_data_type:ident) => {
        pub fn get_annos_mut(&mut self, file_path: &str) -> &mut $tool_data_type {
            if !self.annotations_map.contains_key(file_path) {
                self.annotations_map
                    .insert(file_path.to_string(), $tool_data_type::default());
            }
            self.annotations_map.get_mut(file_path).unwrap()
        }
        pub fn get_annos(&self, file_path: &str) -> &$tool_data_type {
            self.annotations_map
                .get(file_path)
                .unwrap_or(&$default)
        }
        pub fn anno_iter(&self) -> impl Iterator<Item = (&String, &$tool_data_type)> {
            self.annotations_map.iter()
        }
    };
}
