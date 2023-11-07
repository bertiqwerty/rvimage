use crate::{domain::PtF, events::Events, history::History, world::World};

pub trait Manipulate {
    fn new() -> Self
    where
        Self: Sized;

    fn on_activate(&mut self, world: World, history: History) -> (World, History) {
        (world, history)
    }
    fn on_deactivate(&mut self, world: World, history: History) -> (World, History) {
        (world, history)
    }
    /// All events that are used by a tool are implemented in here. Use the macro [`make_tool_transform`](make_tool_transform). See, e.g.,
    /// [`Zoom::events_tf`](crate::tools::Zoom::events_tf).
    fn events_tf(&mut self, world: World, history: History, events: &Events) -> (World, History);
}

#[derive(Clone, Copy, Debug)]
pub struct Mover {
    mouse_pos_start: Option<PtF>,
}
impl Mover {
    pub fn new() -> Self {
        Self {
            mouse_pos_start: None,
        }
    }
    pub fn move_mouse_held<T, F: FnOnce(PtF, PtF) -> T>(
        &mut self,
        f_move: F,
        mouse_pos: Option<PtF>,
    ) -> Option<T> {
        let res = if let (Some(mp_start), Some(mp)) = (self.mouse_pos_start, mouse_pos) {
            let mpo_from = Some(mp_start);
            let mpo_to = Some(mp);
            match (mpo_from, mpo_to) {
                (Some(mpso), Some(mpo)) => Some(f_move(mpso, mpo)),
                _ => None,
            }
        } else {
            None
        };
        self.mouse_pos_start = mouse_pos;
        res
    }
    pub fn move_mouse_pressed(&mut self, mouse_pos: Option<PtF>) {
        if mouse_pos.is_some() {
            self.mouse_pos_start = mouse_pos;
        }
    }
}

// applies the tool transformation to the world
#[macro_export]
macro_rules! make_tool_transform {
    (
        $self:expr,
        $world:expr,
        $history:expr,
        $event:expr,
        [$(($key_event:ident, $key_btn:expr, $method_name:ident)),*]
    ) => {
        if false {
            ($world, $history)
        }
        $(else if $event.$key_event($key_btn) {
            $self.$method_name($event, $world, $history)
        })*
        else {
            ($world, $history)
        }
    };
}
