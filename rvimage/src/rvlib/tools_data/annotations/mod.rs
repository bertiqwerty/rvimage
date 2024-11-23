//! Functionality to create and modify annotations.
pub use self::bbox_annotations::BboxAnnotations;
pub use self::bbox_splitmode::SplitMode;
pub use self::brush_annotations::BrushAnnotations;
pub use self::core::{ClipboardData, InstanceAnnotations};

mod bbox_annotations;
mod bbox_splitmode;
mod brush_annotations;
mod core;
#[macro_export]
macro_rules! implement_annotations_getters {
    ($tool_data_type:ident) => {
        pub fn get_annos_mut(
            &mut self,
            file_path: &str,
            shape_initial: ShapeI,
        ) -> Option<&mut $tool_data_type> {
            if !self.annotations_map.contains_key(file_path) {
                self.annotations_map.insert(
                    file_path.to_string(),
                    ($tool_data_type::default(), shape_initial),
                );
            } else {
                // we reset the shape every time, since a file with the same name but a different shape
                // might have replaced the old one
                if let Some((_, shape)) = self.annotations_map.get_mut(file_path) {
                    if *shape != shape_initial {
                        *shape = shape_initial;
                    }
                }
            }
            self.annotations_map
                .get_mut(file_path)
                .map(|(annos, _)| annos)
        }
        #[must_use]
        pub fn get_annos(&self, file_path: &str) -> Option<&$tool_data_type> {
            let annos = self.annotations_map.get(file_path);
            annos.map(|(annos, _shape)| annos)
        }
        pub fn anno_iter_mut(
            &mut self,
        ) -> impl Iterator<Item = (&String, &mut ($tool_data_type, ShapeI))> {
            self.annotations_map.iter_mut()
        }
        pub fn anno_iter(&self) -> impl Iterator<Item = (&String, &($tool_data_type, ShapeI))> {
            self.annotations_map.iter()
        }
        pub fn anno_intoiter(self) -> impl Iterator<Item = (String, ($tool_data_type, ShapeI))> {
            self.annotations_map.into_iter()
        }
    };
}
