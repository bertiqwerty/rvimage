use crate::domain::PtF;

macro_rules! action_keycode {
    ($name:ident, $action:ident, $key_code:ident) => {
        pub fn $name(&self) -> bool {
            self.events
                .iter()
                .find(|a| match a {
                    Event::$action(KeyCode::$key_code) => true,
                    _ => false,
                })
                .is_some()
        }
    };
}

macro_rules! action {
    ($name:ident, $action:ident) => {
        pub fn $name(&self, key_code: KeyCode) -> bool {
            self.events
                .iter()
                .find(|a| match a {
                    Event::$action(k) => k == &key_code,
                    _ => false,
                })
                .is_some()
        }
    };
}

#[derive(Debug, Clone, Default)]
pub struct Events {
    events: Vec<Event>,
    pub mouse_pos_on_orig: Option<PtF>,
    pub mouse_pos_on_view: Option<PtF>,
}

impl Events {
    pub fn mousepos_orig(mut self, mouse_pos: Option<PtF>) -> Self {
        self.mouse_pos_on_orig = mouse_pos;
        self
    }
    pub fn mousepos_view(mut self, mouse_pos: Option<PtF>) -> Self {
        self.mouse_pos_on_view = mouse_pos;
        self
    }
    pub fn events(mut self, mut events: Vec<Event>) -> Self {
        self.events.append(&mut events);
        self
    }
    action_keycode!(held_alt, Held, Alt);
    action_keycode!(held_shift, Held, Shift);
    action_keycode!(held_ctrl, Held, Ctrl);
    action!(pressed, Pressed);
    action!(held, Held);
    action!(released, Released);
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    E,
    L,
    H,
    I,
    M,
    Q,
    R,
    S,
    T,
    V,
    Y,
    Z,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Plus,
    Minus,
    Delete,
    Back,
    Left,
    Right,
    Up,
    Down,
    F5,
    PageDown,
    PageUp,
    Alt,
    Ctrl,
    Shift,
    Escape,
    MouseLeft,
    MouseRight,
    DontCare,
}

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Pressed(KeyCode),
    Released(KeyCode),
    Held(KeyCode),
    MouseWheel(i64),
}
