use rvimage_domain::{RvResult, ShapeI};

use crate::{
    annotations_accessor_mut,
    events::{Events, KeyCode},
    history::{History, Record},
    make_tool_transform,
    result::trace_ok_err,
    tools_data::{annotations::InstanceAnnotations, rot90_data::NRotations},
    util::Visibility,
    world::World,
    world_annotations_accessor, InstanceAnnotate,
};
use std::mem;

use super::{bbox, brush, Manipulate, BBOX_NAME, BRUSH_NAME};

pub const ACTOR_NAME: &str = "Rot90";
annotations_accessor_mut!(ACTOR_NAME, rot90_mut, "Rotation 90 didn't work", NRotations);
world_annotations_accessor!(ACTOR_NAME, rot90, "Rotation 90 didn't work", NRotations);

fn rot90_instannos_of_file<T>(
    annos: Option<&mut InstanceAnnotations<T>>,
    shape: ShapeI,
) -> RvResult<()>
where
    T: InstanceAnnotate,
{
    if let Some(annos) = annos {
        for elt in annos.elts_iter_mut() {
            *elt = mem::take(elt).rot90_with_image_ntimes(shape, 1)?;
        }
    }
    Ok(())
}

fn rot90_instannos_once(world: &mut World, shape: ShapeI) -> RvResult<()> {
    macro_rules! rot {
        ($name:expr, $module:ident) => {
            let annos = $module::get_annos_mut(world);
            rot90_instannos_of_file(annos, shape)?;
            if let Some(d) = $module::get_options_mut(world) {
                d.core.is_redraw_annos_triggered = true;
            }
            world.request_redraw_annotations($name, Visibility::None);
        };
    }
    rot!(BRUSH_NAME, brush);
    rot!(BBOX_NAME, bbox);
    Ok(())
}

/// rotate 90 degrees counter clockwise
fn rot90(mut world: World, n_rotations: NRotations, skip_annos: bool) -> World {
    let shape = world.data.shape();
    match n_rotations {
        NRotations::Zero => (),
        NRotations::One => {
            if !skip_annos {
                trace_ok_err(rot90_instannos_once(&mut world, shape));
            }
            world.data.apply(|im| im.rotate270());
        }
        NRotations::Two => {
            world.data.apply(|im| im.rotate180());
        }
        NRotations::Three => {
            world.data.apply(|im| im.rotate90());
        }
    }
    world.set_zoom_box(None);
    world.request_redraw_image();
    world
}
#[derive(Clone, Copy, Debug)]
pub struct Rot90;

impl Rot90 {
    fn key_pressed(
        &mut self,
        _events: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        let skip_annos = false;
        world = rot90(world, NRotations::One, skip_annos);
        if let Some(anno) = get_annos_mut(&mut world) {
            *anno = anno.increase();
        }
        history.push(Record::new(world.clone(), ACTOR_NAME));
        (world, history)
    }
}

impl Manipulate for Rot90 {
    fn new() -> Self {
        Self {}
    }

    fn on_filechange(&mut self, mut world: World, history: History) -> (World, History) {
        if let Some(nrot) = get_annos_if_some(&world).copied() {
            let skip_annos = true;
            world = rot90(world, nrot, skip_annos);
        }
        (world, history)
    }

    fn events_tf(&mut self, world: World, history: History, event: &Events) -> (World, History) {
        let (world, history) = make_tool_transform!(
            self,
            world,
            history,
            event,
            [(pressed, KeyCode::R, key_pressed)]
        );
        (world, history)
    }
}
