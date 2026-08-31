//! Tracks which input device owns local control (mouse vs keyboard vs pad).

use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputDevice {
    #[default]
    Keyboard,
    Gamepad,
    Mouse,
}

#[derive(Resource, Default)]
pub struct InputFocus {
    pub device: InputDevice,
    last_mouse_x: f32,
    last_mouse_y: f32,
    /// Sticky mouse move bits — cursor-lost fallback only (not mid-chase thrust).
    pub mouse_move_sticky: u16,
}

/// Visual paddle pose used for mouse aim so online rollback / input-delay
/// cannot make the cursor chase a thrashing sim paddle.
#[derive(Resource, Default, Clone, Copy)]
pub struct MouseAimAnchor {
    pub x: i32,
    pub y: i32,
    pub valid: bool,
}

impl InputFocus {
    pub fn note_keyboard(&mut self) {
        self.device = InputDevice::Keyboard;
    }

    pub fn note_gamepad(&mut self) {
        self.device = InputDevice::Gamepad;
    }

    pub fn note_mouse_at(&mut self, x: f32, y: f32) {
        let dx = x - self.last_mouse_x;
        let dy = y - self.last_mouse_y;
        if dx * dx + dy * dy > 4.0 {
            self.device = InputDevice::Mouse;
        }
        self.last_mouse_x = x;
        self.last_mouse_y = y;
    }
}
