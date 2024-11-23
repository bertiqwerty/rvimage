use crate::{
    cfg::ExportPath,
    meta_data::MetaData,
    result::trace_ok_err,
    tools_data::{merge, ExportAsCoco, ImportMode, Rot90ToolData},
    world::{self, DataAccess, World},
    InstanceAnnotate,
};
use std::mem;

use super::rot90;

pub(super) fn get_rot90_data(world: &World) -> Option<&Rot90ToolData> {
    world::get(world, rot90::ACTOR_NAME, "no rotation_data_found")
        .and_then(|d| d.specifics.rot90())
        .ok()
}
pub fn check_cocoimport<T, A, DA>(
    mut world: World,
    get_specific: impl Fn(&World) -> Option<&T>,
    get_specific_mut: impl Fn(&mut World) -> Option<&mut T>,
    import_coco: impl Fn(&MetaData, &ExportPath, Option<&Rot90ToolData>) -> Option<T>,
) -> (World, bool)
where
    T: ExportAsCoco<A> + Default,
    A: InstanceAnnotate + 'static,
    DA: DataAccess,
{
    enum IsImportTriggered {
        Yes,
        No,
    }
    let options = DA::get_core_options(&world);
    let rot90_data = get_rot90_data(&world);
    let import_info = options.and_then(|options| {
        let import_triggered = if options.import_export_trigger.import_triggered() {
            IsImportTriggered::Yes
        } else {
            IsImportTriggered::No
        };
        get_specific(&world).map(|d| {
            (
                d.cocofile_conn(),
                import_triggered,
                options.import_export_trigger,
            )
        })
    });

    let imported = if let Some((coco_connection, IsImportTriggered::Yes, import_export_trigger)) =
        import_info
    {
        if let Some(imported_data) =
            import_coco(&world.data.meta_data, &coco_connection, rot90_data)
        {
            let (_, import_label_info, import_anno_map, _) = imported_data.separate_data();
            match (
                import_export_trigger.import_mode(),
                get_specific_mut(&mut world),
            ) {
                (ImportMode::Replace, Some(data_mut)) => {
                    data_mut.set_labelinfo(import_label_info);
                    trace_ok_err(data_mut.set_annotations_map(import_anno_map));
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
        data_mut
            .core_options_mut()
            .import_export_trigger
            .untrigger_import();
    }
    (world, imported)
}
