use crate::domain::Point;

macro_rules! action_keycode {
    ($name:ident, $action:ident, $key_code:ident) => {
        pub fn $name(&self) -> bool {
            self.actions
                .iter()
                .find(|a| match a {
                    Action::$action(k) => match k {
                        KeyCode::$key_code => true,
                        _ => false,
                    },
                    _ => false,
                })
                .is_some()
        }
    };
}

macro_rules! action {
    ($name:ident, $action:ident) => {
        pub fn $name(&self, key_code: KeyCode) -> bool {
            self.actions
                .iter()
                .find(|a| match a {
                    Action::$action(k) => k == &key_code,
                    _ => false,
                })
                .is_some()
        }
    };
}

#[derive(Debug, Clone, Default)]
pub struct Events {
    pub actions: Vec<Action>,
    pub mouse_pos: Option<Point>,
}

impl Events {
    action_keycode!(held_alt, Held, Alt);
    action_keycode!(held_shift, Held, Shift);
    action_keycode!(held_ctrl, Held, Ctrl);
    action!(pressed, Pressed);
    action!(held, Held);
    action!(released, Released);
}

#[derive(PartialEq, Debug, Clone)]
pub enum KeyCode {
    A,
    B,
    C,
    D,
    L,
    H,
    M,
    Q,
    R,
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
    Equals,
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
}

#[derive(Debug, Clone)]
pub enum Action {
    Pressed(KeyCode),
    Released(KeyCode),
    Held(KeyCode),
}