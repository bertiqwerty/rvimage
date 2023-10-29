use crate::{
    annotations::BrushAnnotations,
    annotations_accessor, annotations_accessor_mut,
    events::{Events, KeyCode},
    history::{History, Record},
    make_tool_transform,
    tools_data::BrushToolData,
    tools_data::{ToolSpecifics, ToolsData},
    tools_data_initializer,
    world::World,
};

use super::{core::InitialView, Manipulate, BRUSH_NAME};

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
    fn mouse_pressed(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if events.mouse_pos.is_some() {
            get_annos_mut(&mut world).points.push(vec![]);
        }
        (world, history)
    }
    fn mouse_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if let Some(mp) = events.mouse_pos {
            get_annos_mut(&mut world)
                .points
                .last_mut()
                .unwrap()
                .push(mp);
            world.request_redraw_annotations(BRUSH_NAME, true);
        }

        (world, history)
    }

    fn mouse_released(
        &mut self,
        _events: &Events,
        world: World,
        mut history: History,
    ) -> (World, History) {
        history.push(Record::new(world.data.clone(), ACTOR_NAME));
        (world, history)
    }
    fn key_pressed(
        &mut self,
        _events: &Events,
        mut world: World,
        mut history: History,
    ) -> (World, History) {
        get_annos_mut(&mut world).points.clear();
        world.request_redraw_annotations(BRUSH_NAME, true);
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
        events: &Events,
    ) -> (World, History) {
        world = initialize_tools_menu_data(world);
        make_tool_transform!(
            self,
            world,
            history,
            events,
            [
                (pressed, KeyCode::MouseLeft, mouse_pressed),
                (held, KeyCode::MouseLeft, mouse_held),
                (released, KeyCode::MouseLeft, mouse_released),
                (pressed, KeyCode::Back, key_pressed)
            ]
        )
    }
}
