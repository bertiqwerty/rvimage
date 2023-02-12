use std::fmt::Debug;
use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::{
    domain::{zoom_box_mouse_wheel, Shape},
    history::History,
    make_tool_transform,
    tools::core::Manipulate,
    world::World,
    RIGHT_BTN,
};

use super::{core::Mover, zoom::move_zoom_box};

#[derive(Clone, Debug)]
pub struct AlwaysActiveZoom {
    mover: Mover,
}
impl AlwaysActiveZoom {
    fn mouse_pressed(
        &mut self,
        event: &WinitInputHelper,
        _shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        world: World,
        history: History,
    ) -> (World, History) {
        if event.held_control() && event.mouse_pressed(RIGHT_BTN) {
            self.mover.move_mouse_pressed(mouse_pos);
        }
        (world, history)
    }

    fn mouse_held(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if event.held_control() && event.mouse_held(RIGHT_BTN) {
            (self.mover, world) = move_zoom_box(self.mover, world, mouse_pos, shape_win);
            (world, history)
        } else {
            (world, history)
        }
    }

    fn key_released(
        &mut self,
        event: &WinitInputHelper,
        shape_win: Shape,
        _mouse_pos: Option<(usize, usize)>,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if event.held_control() {
            let zb = if event.key_released(VirtualKeyCode::Key0) {
                None
            } else if event.key_released(VirtualKeyCode::Equals) {
                zoom_box_mouse_wheel(*world.zoom_box(), world.shape_orig(), 1.0)
            } else if event.key_released(VirtualKeyCode::Minus) {
                zoom_box_mouse_wheel(*world.zoom_box(), world.shape_orig(), -1.0)
            } else {
                *world.zoom_box()
            };
            world.set_zoom_box(zb, shape_win);
        }
        (world, history)
    }
}
impl Manipulate for AlwaysActiveZoom {
    fn new() -> AlwaysActiveZoom {
        AlwaysActiveZoom {
            mover: Mover::new(),
        }
    }
    fn events_tf(
        &mut self,
        world: World,
        history: History,
        shape_win: Shape,
        mouse_pos: Option<(usize, usize)>,
        event: &WinitInputHelper,
    ) -> (World, History) {
        make_tool_transform!(
            self,
            world,
            history,
            shape_win,
            mouse_pos,
            event,
            [(mouse_pressed, RIGHT_BTN), (mouse_held, RIGHT_BTN)],
            [
                (key_released, VirtualKeyCode::Key0),
                (key_released, VirtualKeyCode::Equals), // Plus is equals
                (key_released, VirtualKeyCode::Minus)
            ]
        )
    }
}
