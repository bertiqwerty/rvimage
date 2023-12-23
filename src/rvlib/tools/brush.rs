use crate::{
    annotations_accessor_mut,
    events::{Events, KeyCode},
    history::{History, Record},
    make_tool_transform,
    tools_data::annotations::BrushAnnotations,
    tools_data::BrushToolData,
    tools_data_initializer,
    world::World,
    Line,
};

use super::{Manipulate, BRUSH_NAME};

const ACTOR_NAME: &str = "Brush";
const MISSING_ANNO_MSG: &str = "brush annotations have not yet been initialized";

tools_data_initializer!(ACTOR_NAME, Brush, BrushToolData);
annotations_accessor_mut!(ACTOR_NAME, brush_mut, MISSING_ANNO_MSG, BrushAnnotations);

#[derive(Clone, Debug)]
pub struct Brush {}

impl Brush {
    fn mouse_pressed(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if let (Some(_), Some(a)) = (events.mouse_pos, get_annos_mut(&mut world)) {
            a.lines.push(Line::new());
        }
        (world, history)
    }
    fn mouse_held(
        &mut self,
        events: &Events,
        mut world: World,
        history: History,
    ) -> (World, History) {
        if let (Some(mp), Some(annos)) = (events.mouse_pos, get_annos_mut(&mut world)) {
            annos.lines.last_mut().unwrap().push(mp.into());
            world.request_redraw_annotations(BRUSH_NAME, true)
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
        if let Some(a) = get_annos_mut(&mut world) {
            a.lines.clear();
            world.request_redraw_annotations(BRUSH_NAME, true);
            history.push(Record::new(world.data.clone(), ACTOR_NAME));
        }
        (world, history)
    }
}

impl Manipulate for Brush {
    fn new() -> Self {
        Self {}
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
