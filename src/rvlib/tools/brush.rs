use crate::{
    annotations::{Annotate, Annotations, BrushAnnotations},
    annotations_accessor, annotations_accessor_mut,
    history::{History, Record},
    make_tool_transform,
    types::ViewImage,
    util::{mouse_pos_to_orig_pos, Shape},
    world::World,
    LEFT_BTN, anno_data_initializer,
};
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::{core::MetaData, Manipulate};

const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";
anno_data_initializer!(ACTOR_NAME, Brush, BrushAnnotations);
annotations_accessor!(ACTOR_NAME, Brush, BrushAnnotations);
annotations_accessor_mut!(ACTOR_NAME, Brush, BrushAnnotations);

#[derive(Clone, Debug)]
pub struct Brush {
    initial_view: Option<ViewImage>,
}

impl Brush {
    fn draw_on_view(&self, mut world: World, shape_win: Shape) -> World {
        let im_view = get_annos(&world)
            .expect(MISSING_ANNO_MSG)
            .brush()
            .draw_on_view(
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
    ) -> (World, History) {
        let mp_orig =
            mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
        if let Some(mp) = mp_orig {
            get_annos_mut(&mut world)
                .brush_mut()
                .points
                .last_mut()
                .unwrap()
                .push(mp);
            world = self.draw_on_view(world, shape_win);
        }
        (world, history)
    }

    fn mouse_pressed(
        &mut self,
        _event: &WinitInputHelper,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        _history: History,
    ) -> (World, History) {
        get_annos_mut(&mut world).brush_mut().points.push(vec![]);
        (world, _history)
    }
    fn mouse_released(
        &mut self,
        _event: &WinitInputHelper,
        _shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        world: World,
        mut history: History,
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
    ) -> (World, History) {
        if world.ims_raw.has_annotations() {
            get_annos_mut(&mut world).brush_mut().points.clear();
            world = self.draw_on_view(world, shape_win);
            history.push(Record::new(world.ims_raw.clone(), ACTOR_NAME));
        }
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
        _meta_data: &MetaData,
    ) -> (World, History) {
        world = initialize_anno_data(world);
        if self.initial_view.is_none() {
            self.initial_view = Some(world.im_view().clone());
        }
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [
                (mouse_pressed, LEFT_BTN),
                (mouse_held, LEFT_BTN),
                (mouse_released, LEFT_BTN)
            ],
            [(key_pressed, VirtualKeyCode::Back)]
        )
    }
}
