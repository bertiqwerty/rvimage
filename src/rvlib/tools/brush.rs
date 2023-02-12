use crate::{
    annotations::BrushAnnotations,
    annotations_accessor, annotations_accessor_mut,
    domain::{mouse_pos_to_orig_pos, Shape},
    history::{History, Record},
    make_tool_transform,
    tools_data::BrushToolData,
    tools_data::{ToolSpecifics, ToolsData},
    tools_data_initializer,
    world::World,
    LEFT_BTN,
};
use egui_winit::winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use super::{core::InitialView, Manipulate};

const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";

tools_data_initializer!(ACTOR_NAME, Brush, BrushToolData);
annotations_accessor_mut!(ACTOR_NAME, brush_mut, MISSING_ANNO_MSG, BrushAnnotations);
annotations_accessor!(ACTOR_NAME, brush, MISSING_ANNO_MSG, BrushAnnotations);

#[derive(Clone, Debug)]
pub struct Brush {
    initial_view: InitialView,
}

impl Brush {
    fn draw_on_view(&self, mut world: World, shape_win: Shape) -> World {
        if let Some(annos) = get_annos(&world) {
            let im_view = annos.draw_on_view(
                self.initial_view.image().clone().unwrap(),
                world.zoom_box(),
                world.data.shape(),
                shape_win,
            );
            world.set_im_view(im_view);
        }
        world
    }

    fn mouse_pressed(
        &mut self,
        _event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        let mp_orig =
            mouse_pos_to_orig_pos(mouse_pos, world.shape_orig(), shape_win, world.zoom_box());
        if mp_orig.is_some() {
            get_annos_mut(&mut world).points.push(vec![]);
        }
        (world, history)
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
                .points
                .last_mut()
                .unwrap()
                .push(mp);
            world = self.draw_on_view(world, shape_win);
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
    ) -> (World, History) {
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
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
        get_annos_mut(&mut world).points.clear();
        world = self.draw_on_view(world, shape_win);
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        (world, history)
    }
}

impl Manipulate for Brush {
    fn new() -> Self {
        Self {
            initial_view: InitialView::new(),
        }
    }

    fn events_tf(
        &mut self,
        mut world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
    ) -> (World, History) {
        self.initial_view.update(&world, shape_win);
        world = initialize_tools_menu_data(world);
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
