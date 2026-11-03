//! SDL scancodes to PC set-1 scan codes, the indices of the original
//! `Keyboard[]` array (`sc_*` in `WOLFSRC/ID_IN.H`). Extended keys map to the
//! code the original saw after dropping the 0xE0 prefix.

use sdl3::keyboard::Scancode as S;

/// PC scan codes used by the game (`sc_*`).
pub mod sc {
    pub const ESCAPE: u8 = 0x01;
    pub const DIGIT_1: u8 = 0x02;
    pub const BACKSPACE: u8 = 0x0E;
    pub const TAB: u8 = 0x0F;
    pub const ENTER: u8 = 0x1C;
    pub const CONTROL: u8 = 0x1D;
    pub const LSHIFT: u8 = 0x2A;
    pub const RSHIFT: u8 = 0x36;
    pub const ALT: u8 = 0x38;
    pub const SPACE: u8 = 0x39;
    pub const CAPSLOCK: u8 = 0x3A;
    pub const F1: u8 = 0x3B;
    pub const F9: u8 = 0x43;
    pub const F10: u8 = 0x44;
    pub const F11: u8 = 0x57;
    pub const F12: u8 = 0x58;
    pub const HOME: u8 = 0x47;
    pub const UP: u8 = 0x48;
    pub const PGUP: u8 = 0x49;
    pub const LEFT: u8 = 0x4B;
    pub const RIGHT: u8 = 0x4D;
    pub const END: u8 = 0x4F;
    pub const DOWN: u8 = 0x50;
    pub const PGDN: u8 = 0x51;
    pub const INSERT: u8 = 0x52;
    pub const DELETE: u8 = 0x53;
    pub const PAUSE: u8 = 0x7F;
    pub const A: u8 = 0x1E;
    pub const D: u8 = 0x20;
    pub const E: u8 = 0x12;
    pub const H: u8 = 0x23;
    pub const I: u8 = 0x17;
    pub const M: u8 = 0x32;
    pub const N: u8 = 0x31;
    pub const Q: u8 = 0x10;
    pub const S: u8 = 0x1F;
    pub const W: u8 = 0x11;
    pub const Y: u8 = 0x15;
}

/// Maps an SDL scancode to its PC set-1 code.
pub fn to_pc(s: S) -> Option<u8> {
    Some(match s {
        S::Escape => 0x01,
        S::_1 => 0x02,
        S::_2 => 0x03,
        S::_3 => 0x04,
        S::_4 => 0x05,
        S::_5 => 0x06,
        S::_6 => 0x07,
        S::_7 => 0x08,
        S::_8 => 0x09,
        S::_9 => 0x0A,
        S::_0 => 0x0B,
        S::Minus => 0x0C,
        S::Equals => 0x0D,
        S::Backspace => 0x0E,
        S::Tab => 0x0F,
        S::Q => 0x10,
        S::W => 0x11,
        S::E => 0x12,
        S::R => 0x13,
        S::T => 0x14,
        S::Y => 0x15,
        S::U => 0x16,
        S::I => 0x17,
        S::O => 0x18,
        S::P => 0x19,
        S::LeftBracket => 0x1A,
        S::RightBracket => 0x1B,
        S::Return | S::KpEnter => 0x1C,
        S::LCtrl | S::RCtrl => 0x1D,
        S::A => 0x1E,
        S::S => 0x1F,
        S::D => 0x20,
        S::F => 0x21,
        S::G => 0x22,
        S::H => 0x23,
        S::J => 0x24,
        S::K => 0x25,
        S::L => 0x26,
        S::Semicolon => 0x27,
        S::Apostrophe => 0x28,
        S::Grave => 0x29,
        S::LShift => 0x2A,
        S::Backslash => 0x2B,
        S::Z => 0x2C,
        S::X => 0x2D,
        S::C => 0x2E,
        S::V => 0x2F,
        S::B => 0x30,
        S::N => 0x31,
        S::M => 0x32,
        S::Comma => 0x33,
        S::Period => 0x34,
        S::Slash | S::KpDivide => 0x35,
        S::RShift => 0x36,
        S::KpMultiply => 0x37,
        S::LAlt | S::RAlt => 0x38,
        S::Space => 0x39,
        S::CapsLock => 0x3A,
        S::F1 => 0x3B,
        S::F2 => 0x3C,
        S::F3 => 0x3D,
        S::F4 => 0x3E,
        S::F5 => 0x3F,
        S::F6 => 0x40,
        S::F7 => 0x41,
        S::F8 => 0x42,
        S::F9 => 0x43,
        S::F10 => 0x44,
        S::NumLockClear => 0x45,
        S::ScrollLock => 0x46,
        S::Home | S::Kp7 => 0x47,
        S::Up | S::Kp8 => 0x48,
        S::PageUp | S::Kp9 => 0x49,
        S::KpMinus => 0x4A,
        S::Left | S::Kp4 => 0x4B,
        S::Kp5 => 0x4C,
        S::Right | S::Kp6 => 0x4D,
        S::KpPlus => 0x4E,
        S::End | S::Kp1 => 0x4F,
        S::Down | S::Kp2 => 0x50,
        S::PageDown | S::Kp3 => 0x51,
        S::Insert | S::Kp0 => 0x52,
        S::Delete | S::KpPeriod => 0x53,
        S::F11 => 0x57,
        S::F12 => 0x58,
        S::Pause => sc::PAUSE,
        _ => return None,
    })
}
