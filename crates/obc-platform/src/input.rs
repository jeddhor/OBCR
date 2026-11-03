//! Abstract input state collected from SDL events.

/// Gamepad button bits for [`GamepadState::buttons`].
pub mod pad {
    pub const SOUTH: u32 = 1 << 0;
    pub const EAST: u32 = 1 << 1;
    pub const WEST: u32 = 1 << 2;
    pub const NORTH: u32 = 1 << 3;
    pub const BACK: u32 = 1 << 4;
    pub const START: u32 = 1 << 5;
    pub const LEFT_STICK: u32 = 1 << 6;
    pub const RIGHT_STICK: u32 = 1 << 7;
    pub const LEFT_SHOULDER: u32 = 1 << 8;
    pub const RIGHT_SHOULDER: u32 = 1 << 9;
    pub const DPAD_UP: u32 = 1 << 10;
    pub const DPAD_DOWN: u32 = 1 << 11;
    pub const DPAD_LEFT: u32 = 1 << 12;
    pub const DPAD_RIGHT: u32 = 1 << 13;
}

/// Combined state of all connected gamepads. Sticks are -1..=1, triggers 0..=1.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GamepadState {
    pub connected: bool,
    pub left_x: f32,
    pub left_y: f32,
    pub right_x: f32,
    pub right_y: f32,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons: u32,
}

impl GamepadState {
    pub fn held(&self, bits: u32) -> bool {
        self.buttons & bits != 0
    }
}

/// A key transition, by original PC scan code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub code: u8,
    pub down: bool,
    pub repeat: bool,
}

/// Everything the game reads from the user.
#[derive(Debug, Clone)]
pub struct InputState {
    /// Held keys by PC set-1 scan code (`Keyboard[]` in `WOLFSRC/ID_IN.C`).
    pub keys: [bool; 128],
    /// Key transitions since the last [`InputState::end_frame`].
    pub events: Vec<KeyEvent>,
    /// Text typed since the last frame (when text input is enabled).
    pub text: String,
    /// Relative mouse motion since the last frame, in pixels.
    pub mouse_dx: f32,
    pub mouse_dy: f32,
    /// Held mouse buttons: bit 0 left, bit 1 middle, bit 2 right.
    pub mouse_buttons: u8,
    /// Gamepad state at the last pump.
    pub pad: GamepadState,
    /// Gamepad buttons at the previous frame (for edge detection).
    pub pad_prev: u32,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys: [false; 128],
            events: Vec::new(),
            text: String::new(),
            mouse_dx: 0.0,
            mouse_dy: 0.0,
            mouse_buttons: 0,
            pad: GamepadState::default(),
            pad_prev: 0,
        }
    }
}

impl InputState {
    pub(crate) fn key(&mut self, code: u8, down: bool, repeat: bool) {
        if let Some(k) = self.keys.get_mut(code as usize) {
            *k = down;
        }
        self.events.push(KeyEvent { code, down, repeat });
    }

    pub(crate) fn mouse_button(&mut self, button: u8, down: bool) {
        // SDL: 1 left, 2 middle, 3 right.
        let bit = match button {
            1 => 1,
            2 => 2,
            3 => 4,
            _ => return,
        };
        if down {
            self.mouse_buttons |= bit;
        } else {
            self.mouse_buttons &= !bit;
        }
    }

    pub(crate) fn release_all(&mut self) {
        self.keys = [false; 128];
        self.mouse_buttons = 0;
    }

    pub fn down(&self, code: u8) -> bool {
        self.keys.get(code as usize).copied().unwrap_or(false)
    }

    /// True if `code` was pressed (not repeated) since the last frame.
    pub fn pressed(&self, code: u8) -> bool {
        self.events.iter().any(|e| e.code == code && e.down && !e.repeat)
    }

    /// True if a gamepad button in `bits` went down since the last frame.
    pub fn pad_pressed(&self, bits: u32) -> bool {
        self.pad.buttons & bits & !self.pad_prev != 0
    }

    /// Clears per-frame data (call after the frame's tics consumed it).
    pub fn end_frame(&mut self) {
        self.events.clear();
        self.text.clear();
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.pad_prev = self.pad.buttons;
    }
}
