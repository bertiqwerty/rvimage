use crate::{
    InstanceAnnotate,
    cfg::ExportPath,
    history::{History, Record},
    meta_data::MetaData,
    result::trace_ok_err,
    tools::{
        bbox, brush,
        wand::{AnnosWithInfo, ImageForPrediction, RestWand, Wand, WandAnnotationsInput},
    },
    tools_data::{ExportAsCoco, ImportMode, Rot90ToolData, merge},
    world::{self, MetaDataAccess, World},
};
use std::{
    mem,
    path::Path,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

use super::rot90;

pub(super) fn get_rot90_data(world: &World) -> Option<&Rot90ToolData> {
    world::get(world, rot90::ACTOR_NAME, "no rotation_data_found")
        .and_then(|d| d.specifics.rot90())
        .ok()
}

pub fn predictive_labeling<DA>(
    world: &mut World,
    history: &mut History,
    actor: &'static str,
    prediction_receiver: &mut Option<Receiver<(World, History)>>,
) where
    DA: MetaDataAccess,
{
    let pred_data = DA::get_predictive_labeling_data(world);
    if let Some(pred_data) = pred_data
        && pred_data.prediction_start_triggered()
    {
        tracing::info!("Predictive labeling thread is spawned");
        let mut world = world.clone();
        if let Some(pred_data_mut) = DA::get_predictive_labeling_data_mut(&mut world) {
            pred_data_mut.untrigger();
        }
        let mut history = history.clone();
        let url = pred_data.url.clone();
        let parameters = pred_data.parameters.clone();
        let timeout_ms = pred_data.timeout_ms;
        let (tx, rx) = mpsc::channel();
        let pred_thread = move || {
            let wand = RestWand::new(url, None, timeout_ms);
            let im = ImageForPrediction {
                image: world.data.im_background(),
                path: world.data.meta_data.file_path_absolute().map(Path::new),
            };
            let bbox_annos = bbox::get_annos_if_some(&world);
            let brush_annos = brush::get_annos_if_some(&world);
            let bbox_label_info = bbox::get_label_info(&world);
            let brush_label_info = brush::get_label_info(&world);

            let predictions = trace_ok_err(wand.predict(
                im,
                actor,
                Some(&parameters),
                WandAnnotationsInput {
                    bbox: bbox_annos.and_then(|annos| {
                        bbox_label_info.map(|labelinfo| AnnosWithInfo { labelinfo, annos })
                    }),
                    brush: brush_annos.and_then(|annos| {
                        brush_label_info.map(|labelinfo| AnnosWithInfo { labelinfo, annos })
                    }),
                },
                *world.zoom_box(),
            ));
            if let Some(pred) = predictions {
                tracing::info!(
                    "received {:?} bbox predictions",
                    pred.bbox.as_ref().map(|ia| ia.len())
                );
                tracing::info!(
                    "received {:?} brush predictions",
                    pred.brush.as_ref().map(|ia| ia.len())
                );
                if let (Some(pred), Some(annos)) = (pred.bbox, bbox::get_annos_mut(&mut world)) {
                    *annos = pred;
                }
                if let (Some(pred), Some(annos)) = (pred.brush, brush::get_annos_mut(&mut world)) {
                    *annos = pred;
                }
                history.push(Record::new(world.clone(), actor));
            }
            tracing::info!("Predictive labeling thread sending data");
            trace_ok_err(tx.send((world, history)));
        };
        thread::spawn(pred_thread);
        *prediction_receiver = Some(rx);
    }
    if let Some(pred_data_mut) = DA::get_predictive_labeling_data_mut(world) {
        pred_data_mut.untrigger();
        if let Some(rx) = prediction_receiver {
            match rx.try_recv() {
                Ok((world_pred, history_pred)) => {
                    pred_data_mut.kill_trigger();
                    *prediction_receiver = None;
                    *world = world_pred;
                    *history = history_pred;
                    world.request_redraw_annotations(actor, crate::util::Visibility::All);
                    tracing::info!("received prediction from predictive labeling of {actor}");
                }
                Err(TryRecvError::Empty) => {
                    if let Some(t) = pred_data_mut.trigger_time()
                        && t.elapsed().as_millis() as usize > pred_data_mut.timeout_ms
                    {
                        tracing::error!(
                            "timeout of predictive labeling of {actor} after{} ms",
                            pred_data_mut.timeout_ms
                        );
                        pred_data_mut.kill_trigger();
                        *prediction_receiver = None;
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    tracing::error!("prediction receiver disconnected for {actor}");
                }
            }
        }
    };
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
    DA: MetaDataAccess,
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
