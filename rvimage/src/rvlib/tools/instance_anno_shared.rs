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
    import_coco: impl Fn(&MetaData, &ExportPath, Option<&Rot90ToolData>) -> Option<T>,
) -> (World, bool)
where
    T: ExportAsCoco<A> + Default,
    A: InstanceAnnotate + 'static,
{
    let options = get_options(&world);
    let rot90_data = get_rot90_data(&world);
    let import_info = if let Some(options) = options {
        get_specific(&world).map(|d| (d.cocofile_conn(), options.import_mode))
    } else {
        None
    };
    let imported = if let Some((coco_connection, import_mode)) = import_info {
        if let Some(imported_data) =
            import_coco(&world.data.meta_data, &coco_connection, rot90_data)
        {
            let (_, import_label_info, import_anno_map, _) = imported_data.separate_data();
            match (import_mode, get_specific_mut(&mut world)) {
                (ImportMode::Replace, Some(data_mut)) => {
                    trace_ok_err(data_mut.set_annotations_map(import_anno_map));
                    data_mut.set_labelinfo(import_label_info);
                }
                (ImportMode::Merge, Some(data_mut)) => {
                    let (options, label_info, anno_map, export_path) =
                        mem::take(data_mut).separate_data();
                    let (annotations_map, label_info) =
                        merge(anno_map, label_info, import_anno_map, import_label_info);
                    *data_mut = T::new(options, label_info, annotations_map, export_path);
                }
                _ => (),
            }
            true
        } else {
            false
        }
    } else {
        false
    };
    if let Some(data_mut) = get_specific_mut(&mut world) {
        data_mut.core_options_mut().is_import_triggered = false;
    }
    (world, imported)
}
