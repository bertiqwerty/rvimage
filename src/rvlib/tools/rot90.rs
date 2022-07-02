use image::imageops::{self, FilterType};
use winit::event::VirtualKeyCode;

use crate::{
    history::{History, Record},
    make_tool_transform,
    util::{self, Event, Shape},
    world::{ImsRaw, World}, types::ViewImage,
};

use super::Manipulate;

pub fn scale_to_win(ims_raw: &ImsRaw, shape_win: Shape) -> ViewImage {
    let shape_orig = ims_raw.shape();
    let new = util::shape_scaled(shape_orig, shape_win);
    let im_view = ims_raw.to_view();
    imageops::resize(&im_view, new.w, new.h, FilterType::Nearest)
}
/// rotate 90 degrees counter clockwise
fn rot90(ims: &ImsRaw) -> ImsRaw {
    let mut ims = ims.clone();
    ims.apply(
        |im| im.rotate270(),
        |mask| mask.as_ref().map(imageops::rotate270),
    );
    ims
}
#[derive(Clone, Copy, Debug)]
pub struct Rot90;

impl Rot90 {
    fn key_pressed(
        &mut self,
        key: VirtualKeyCode,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        if key == VirtualKeyCode::R {
            history.push(Record {
                ims_raw: world.ims_raw().clone(),
                file_label_idx: None,
                folder_label: None,
            });
            world = World::new(rot90(world.ims_raw()));
            *world.im_view_mut() = scale_to_win(world.ims_raw(), shape_win);
        }
        (world, history)
    }
}

impl Manipulate for Rot90 {
    fn new() -> Self {
        Self {}
    }

    fn events_tf<'a>(
        &'a mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &Event,
    ) -> (World, History) {
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [],
            [VirtualKeyCode::R]
        )
    }
}
