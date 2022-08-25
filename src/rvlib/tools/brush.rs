use crate::{
    anno_data_initializer,
    annotations::{Annotate, Annotations, BrushAnnotations},
    annotations_accessor, annotations_accessor_mut,
    history::{History, Record},
    make_tool_transform,
    types::ViewImage,
    util::{mouse_pos_to_orig_pos, Shape},
    world::World,
    LEFT_BTN,
};
use std::collections::HashMap;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::{core::MetaData, Manipulate};

const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";
anno_data_initializer!(ACTOR_NAME, Brush, BrushAnnotations);
annotations_accessor!(ACTOR_NAME, Brush, BrushAnnotations, MISSING_ANNO_MSG);
annotations_accessor_mut!(ACTOR_NAME, Brush, BrushAnnotations, MISSING_ANNO_MSG);

#[derive(Clone, Debug)]
pub struct Brush {
    initial_view: Option<ViewImage>,
}

impl Brush {
    fn draw_on_view(
        &self,
        mut world: World,
        shape_win: Shape,
        current_file_path: Option<&str>,
    ) -> World {
        let im_view = get_annos(&world, current_file_path).brush().draw_on_view(
            self.initial_view.clone().unwrap(),
            world.zoom_box(),
            world.ims_raw.shape(),
            shape_win,
        );
        world.set_im_view(im_view);
        world
    }
    fn mouse_held(
        &mut self,
        _event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
        meta_data: &MetaData,
    ) -> (World, History) {
        let mp_orig =
            mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
        if let Some(mp) = mp_orig {
            get_annos_mut(&mut world, meta_data.file_path)
                .brush_mut()
                .points
                .last_mut()
                .unwrap()
                .push(mp);
            world = self.draw_on_view(world, shape_win, meta_data.file_path);
        }
        (world, history)
    }

    fn mouse_released(
        &mut self,
        _event: &WinitInputHelper,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        world: World,
        mut history: History,
        _meta_data: &MetaData,
    ) -> (World, History) {
        history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
        (world, history)
    }
    fn key_pressed(
        &mut self,
        _event: &WinitInputHelper,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        mut history: History,
        meta_data: &MetaData,
    ) -> (World, History) {
        get_annos_mut(&mut world, meta_data.file_path)
            .brush_mut()
            .points
            .clear();
        world = self.draw_on_view(world, shape_win, meta_data.file_path);
        history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
        (world, history)
    }
}

impl Manipulate for Brush {
    fn new() -> Self {
        Self { initial_view: None }
    }

    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
        meta_data: &MetaData,
    ) -> (World, History) {
        world = initialize_anno_data(world, meta_data.file_path);
        if self.initial_view.is_none() {
            self.initial_view = Some(
                world
                    .ims_raw
                    .bg_to_unannotated_view(world.zoom_box(), shape_win),
            );
        }
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            meta_data,
            [(mouse_held, LEFT_BTN), (mouse_released, LEFT_BTN)],
            [(key_pressed, VirtualKeyCode::Back)]
        )
    }
}
