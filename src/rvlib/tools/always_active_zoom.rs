use std::fmt::Debug;

use crate::{
    domain::zoom_box_mouse_wheel,
    events::{Events, KeyCode},
    history::History,
    make_tool_transform,
    tools::core::Manipulate,
    world::World,
};

use super::{core::Mover, zoom::move_zoom_box};

fn event_move_zoom_box(events: &Events) -> bool {
    events.held_ctrl() && events.pressed(KeyCode::MouseLeft)
}

#[derive(Clone, Debug)]
pub struct AlwaysActiveZoom {
    mover: Mover,
}
impl AlwaysActiveZoom {
    fn mouse_pressed(
        &mut self,
        events: &Events,
        world: World,
        history: History,
    ) -> (World, History) {
        if event_move_zoom_box(events) {
            self.mover.move_mouse_pressed(events.mouse_pos_on_view);
        }
        (world, history)
    }

    fn mouse_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if event_move_zoom_box(events) {
            (self.mover, world) = move_zoom_box(self.mover, world, events.mouse_pos_on_view);
            (world, history)
        } else {
            (world, history)
        }
    }

    fn key_released(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if events.held_ctrl() {
            let zb = if events.released(KeyCode::Key0) {
                None
            } else if events.released(KeyCode::PlusEquals) {
                zoom_box_mouse_wheel(*world.zoom_box(), world.shape_orig(), 1.0)
            } else if events.released(KeyCode::Minus) {
                zoom_box_mouse_wheel(*world.zoom_box(), world.shape_orig(), -1.0)
            } else {
                *world.zoom_box()
            };
            world.set_zoom_box(zb);
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
    fn has_been_used(&self, events: &Events) -> Option<bool> {
        let zoomed = events.held_ctrl()
            && (events.released(KeyCode::Key0)
                || events.released(KeyCode::PlusEquals)
                || events.released(KeyCode::Minus));
        Some(zoomed || event_move_zoom_box(events))
    }
    fn events_tf(&mut self, world: World, history: History, events: &Events) -> (World, History) {
        make_tool_transform!(
            self,
            world,
            history,
            events,
            [
                (pressed, KeyCode::MouseLeft, mouse_pressed),
                (held, KeyCode::MouseLeft, mouse_held),
                (released, KeyCode::Key0, key_released),
                (released, KeyCode::PlusEquals, key_released), // Plus is equals
                (released, KeyCode::Minus, key_released)
            ]
        )
    }
}
