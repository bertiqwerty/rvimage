use crate::{Event, KeyCode, ZoomAmount};

#[derive(Debug, Default)]
pub struct LastSensedBtns {
    pub btn_codes: Vec<KeyCode>,
    pub modifiers: Vec<Event>,
}
impl LastSensedBtns {
    pub fn is_empty(&self) -> bool {
        self.btn_codes.is_empty() && self.modifiers.is_empty()
    }
}
pub fn map_key(egui_key: egui::Key) -> Option<KeyCode> {
    match egui_key {
        egui::Key::A => Some(KeyCode::A),
        egui::Key::B => Some(KeyCode::B),
        egui::Key::C => Some(KeyCode::C),
        egui::Key::D => Some(KeyCode::D),
        egui::Key::E => Some(KeyCode::E),
        egui::Key::L => Some(KeyCode::L),
        egui::Key::H => Some(KeyCode::H),
        egui::Key::I => Some(KeyCode::I),
        egui::Key::M => Some(KeyCode::M),
        egui::Key::Q => Some(KeyCode::Q),
        egui::Key::R => Some(KeyCode::R),
        egui::Key::S => Some(KeyCode::S),
        egui::Key::T => Some(KeyCode::T),
        egui::Key::V => Some(KeyCode::V),
        egui::Key::Y => Some(KeyCode::Y),
        egui::Key::Z => Some(KeyCode::Z),
        egui::Key::Num0 => Some(KeyCode::Key0),
        egui::Key::Num1 => Some(KeyCode::Key1),
        egui::Key::Num2 => Some(KeyCode::Key2),
        egui::Key::Num3 => Some(KeyCode::Key3),
        egui::Key::Num4 => Some(KeyCode::Key4),
        egui::Key::Num5 => Some(KeyCode::Key5),
        egui::Key::Num6 => Some(KeyCode::Key6),
        egui::Key::Num7 => Some(KeyCode::Key7),
        egui::Key::Num8 => Some(KeyCode::Key8),
        egui::Key::Num9 => Some(KeyCode::Key9),
        egui::Key::Plus | egui::Key::Equals => Some(KeyCode::PlusEquals),
        egui::Key::Minus => Some(KeyCode::Minus),
        egui::Key::Delete => Some(KeyCode::Delete),
        egui::Key::Backspace => Some(KeyCode::Back),
        egui::Key::ArrowLeft => Some(KeyCode::Left),
        egui::Key::ArrowRight => Some(KeyCode::Right),
        egui::Key::ArrowUp => Some(KeyCode::Up),
        egui::Key::ArrowDown => Some(KeyCode::Down),
        egui::Key::F5 => Some(KeyCode::F5),
        egui::Key::PageDown => Some(KeyCode::PageDown),
        egui::Key::PageUp => Some(KeyCode::PageUp),
        egui::Key::Escape => Some(KeyCode::Escape),
        _ => None,
    }
}
pub fn map_modifiers(modifiers: egui::Modifiers) -> Vec<Event> {
    let mut events = Vec::new();
    if modifiers.alt {
        events.push(Event::Held(KeyCode::Alt));
    }
    if modifiers.command || modifiers.ctrl {
        events.push(Event::Held(KeyCode::Ctrl));
    }
    if modifiers.shift {
        events.push(Event::Held(KeyCode::Shift));
    }
    events
}

pub fn map_key_events(ui: &mut egui::Ui) -> Vec<Event> {
    let mut events = vec![];
    ui.input(|i| {
        for e in &i.events {
            if let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                physical_key: _,
            } = e
            {
                if let Some(k) = map_key(*key) {
                    if !pressed {
                        events.push(Event::Released(k));
                    } else if !repeat {
                        events.push(Event::Pressed(k));
                        events.push(Event::Held(k));
                    } else {
                        events.push(Event::Held(k));
                    }
                }
                let mut modifier_events = map_modifiers(*modifiers);
                events.append(&mut modifier_events);
            }
        }
    });
    events
}

pub fn map_mouse_events(
    ui: &mut egui::Ui,
    last_sensed: &mut LastSensedBtns,
    image_response: &egui::Response,
) -> Vec<Event> {
    let mut events = vec![];
    let mut btn_codes = LastSensedBtns::default();
    ui.input(|i| {
        for e in &i.events {
            match e {
                egui::Event::PointerButton {
                    pos: _,
                    button,
                    pressed: _,
                    modifiers,
                } => {
                    let modifier_events = map_modifiers(*modifiers);
                    let btn_code = match button {
                        egui::PointerButton::Primary => KeyCode::MouseLeft,
                        egui::PointerButton::Secondary => KeyCode::MouseRight,
                        _ => KeyCode::DontCare,
                    };
                    btn_codes.btn_codes.push(btn_code);
                    btn_codes.modifiers = modifier_events;
                }
                egui::Event::Zoom(z) => {
                    events.push(Event::Zoom(ZoomAmount::Factor(f64::from(*z))));
                }
                egui::Event::MouseWheel {
                    unit: _,
                    delta,
                    modifiers,
                } => {
                    if modifiers.ctrl {
                        events.push(Event::Zoom(ZoomAmount::Delta(f64::from(delta.y))));
                    }
                }
                _ => {}
            };
        }
    });
    if !btn_codes.is_empty() {
        *last_sensed = btn_codes;
    }

    if image_response.clicked()
        || image_response.secondary_clicked()
        || image_response.drag_stopped()
    {
        if last_sensed.btn_codes.contains(&KeyCode::MouseLeft) {
            events.push(Event::Released(KeyCode::MouseLeft));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        } else if last_sensed.btn_codes.contains(&KeyCode::MouseRight) {
            events.push(Event::Released(KeyCode::MouseRight));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        }
        *last_sensed = LastSensedBtns::default();
    }
    if image_response.drag_started() {
        if last_sensed.btn_codes.contains(&KeyCode::MouseLeft) {
            events.push(Event::Pressed(KeyCode::MouseLeft));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        } else if last_sensed.btn_codes.contains(&KeyCode::MouseRight) {
            events.push(Event::Pressed(KeyCode::MouseRight));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        }
    }
    if image_response.dragged() {
        if last_sensed.btn_codes.contains(&KeyCode::MouseLeft) {
            events.push(Event::Held(KeyCode::MouseLeft));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        } else if last_sensed.btn_codes.contains(&KeyCode::MouseRight) {
            events.push(Event::Held(KeyCode::MouseRight));
            for modifier in &last_sensed.modifiers {
                events.push(*modifier);
            }
        }
    }
    events
}
