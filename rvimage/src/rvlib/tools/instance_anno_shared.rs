use crate::{
    cfg::ExportPath,
    meta_data::MetaData,
    result::trace_ok_err,
    tools_data::{self, merge, CoreOptions, ExportAsCoco, ImportMode, Rot90ToolData},
    world::World,
    InstanceAnnotate,
};
use std::mem;

use super::rot90;

pub(super) fn get_rot90_data(world: &World) -> Option<&Rot90ToolData> {
    tools_data::get(world, rot90::ACTOR_NAME, "no rotation_data_found")
        .and_then(|d| d.specifics.rot90())
        .ok()
}
pub fn check_cocoimport<T, A>(
    mut world: World,
    get_options: impl Fn(&World) -> Option<CoreOptions>,
    get_rot90_data: impl Fn(&World) -> Option<&Rot90ToolData>,
    get_specific: impl Fn(&World) -> Option<&T>,
    get_specific_mut: impl Fn(&mut World) -> Option<&mut T>,
    import_coco_if_triggered: impl Fn(
        &MetaData,
        Option<&ExportPath>,
        Option<&Rot90ToolData>,
    ) -> Option<T>,
) -> (World, bool)
where
    T: ExportAsCoco<A> + Default,
    A: InstanceAnnotate + 'static,
{
    // import coco if demanded
    let mut imported = false;
    let options = get_options(&world);
    if let Some(options) = options {
        let rot90_data = get_rot90_data(&world);
        let d = if options.is_import_triggered {
            get_specific(&world).map(|o| o.cocofile_conn())
        } else {
            None
        };
        if let Some(imported_data) =
            import_coco_if_triggered(&world.data.meta_data, d.as_ref(), rot90_data)
        {
            if let Some(data_mut) = get_specific_mut(&mut world) {
                if options.is_import_triggered {
                    let (_, import_label_info, import_anno_map, _) = imported_data.separate_data();
                    match options.import_mode {
                        ImportMode::Replace => {
                            trace_ok_err(data_mut.set_annotations_map(import_anno_map));
                            data_mut.set_labelinfo(import_label_info);
                        }
                        ImportMode::Merge => {
                            let (options, label_info, anno_map, export_path) =
                                mem::take(data_mut).separate_data();
                            let (annotations_map, label_info) =
                                merge(anno_map, label_info, import_anno_map, import_label_info);
                            *data_mut = T::new(options, label_info, annotations_map, export_path);
                        }
                    }
                    data_mut.core_options_mut().is_import_triggered = false;
                }
                imported = true;
            }
        } else if let Some(data_mut) = get_specific_mut(&mut world) {
            data_mut.core_options_mut().is_import_triggered = false;
        }
    }
    (world, imported)
}
