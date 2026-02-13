use gb_core::joypad::Button;
use winit::keyboard::{KeyCode, PhysicalKey};

/// Map a winit physical key to a Game Boy button.
pub fn map_key(key: PhysicalKey) -> Option<Button> {
    match key {
        PhysicalKey::Code(code) => match code {
            // D-pad
            KeyCode::ArrowRight => Some(Button::Right),
            KeyCode::ArrowLeft => Some(Button::Left),
            KeyCode::ArrowUp => Some(Button::Up),
            KeyCode::ArrowDown => Some(Button::Down),
            // Action buttons
            KeyCode::KeyZ => Some(Button::A),
            KeyCode::KeyX => Some(Button::B),
            // System buttons
            KeyCode::Enter => Some(Button::Start),
            KeyCode::Backspace => Some(Button::Select),
            _ => None,
        },
        _ => None,
    }
}
