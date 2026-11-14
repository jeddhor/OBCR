//! Rebindable controls (issue #2): every game action can take up to
//! [`MAX_KEYS`] keyboard or mouse inputs and [`MAX_PAD`] gamepad buttons.
//!
//! The two control schemes (`modern`, `original`) differ in mouse and run
//! behaviour (`controls.rs`) and in their default bindings here. Bindings are
//! saved in `obcr.toml` as readable names (`fire = ["LEFT CTRL", "MOUSE LEFT",
//! "PAD RT"]`). Analog sticks and mouse turning are not bindings: they always
//! move and turn, adjusted by the speed settings.

use std::collections::BTreeMap;

use obc_core::input::{InputState, pad};
use serde::{Deserialize, Serialize};

/// Keyboard or mouse inputs per action.
pub const MAX_KEYS: usize = 3;
/// Gamepad buttons per action.
pub const MAX_PAD: usize = 2;

/// Something the player can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Forward,
    Back,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
    /// Held: the turn keys strafe instead (the original's Alt).
    Strafe,
    Run,
    Fire,
    Use,
    Weapon1,
    Weapon2,
    Weapon3,
    Weapon4,
    Weapon5,
    NextWeapon,
    PrevWeapon,
    AllySwitch,
    AllyStay,
    AllyMode,
    WipeVisor,
    Map,
    Crosshair,
    Health,
    Tremors,
    Pause,
    QuickSave,
    SoundToggle,
    MusicToggle,
    FloorsOn,
    FloorsOff,
    Menu,
    Quit,
}

/// Display order, with labels.
pub const ACTIONS: [(Action, &str); 33] = [
    (Action::Forward, "FORWARD"),
    (Action::Back, "BACKWARD"),
    (Action::TurnLeft, "TURN LEFT"),
    (Action::TurnRight, "TURN RIGHT"),
    (Action::StrafeLeft, "STRAFE LEFT"),
    (Action::StrafeRight, "STRAFE RIGHT"),
    (Action::Strafe, "STRAFE HOLD"),
    (Action::Run, "RUN"),
    (Action::Fire, "FIRE"),
    (Action::Use, "USE / OPEN"),
    (Action::Weapon1, "WEAPON 1"),
    (Action::Weapon2, "WEAPON 2"),
    (Action::Weapon3, "WEAPON 3"),
    (Action::Weapon4, "WEAPON 4"),
    (Action::Weapon5, "WEAPON 5"),
    (Action::NextWeapon, "NEXT WEAPON"),
    (Action::PrevWeapon, "PREV WEAPON"),
    (Action::AllySwitch, "ALLY SWITCH"),
    (Action::AllyStay, "STAY+SWITCH"),
    (Action::AllyMode, "ALLY MODE"),
    (Action::WipeVisor, "WIPE VISOR"),
    (Action::Map, "MAP"),
    (Action::Crosshair, "CROSSHAIR"),
    (Action::Health, "HEALTH"),
    (Action::Tremors, "TREMORS"),
    (Action::Pause, "PAUSE"),
    (Action::QuickSave, "QUICK SAVE"),
    (Action::SoundToggle, "SOUND"),
    (Action::MusicToggle, "MUSIC"),
    (Action::FloorsOn, "FLOORS ON"),
    (Action::FloorsOff, "FLOORS OFF"),
    (Action::Menu, "MENU"),
    (Action::Quit, "QUIT TO DOS"),
];

pub fn label(a: Action) -> &'static str {
    ACTIONS.iter().find(|(x, _)| *x == a).map_or("?", |(_, l)| l)
}

/// One physical input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bind {
    /// PC set-1 scan code.
    Key(u8),
    /// Mouse button bit of [`InputState::mouse_buttons`].
    Mouse(u8),
    WheelUp,
    WheelDown,
    /// Gamepad button bit of [`obc_core::input::GamepadState::buttons`].
    Pad(u32),
}

impl Bind {
    pub fn is_pad(self) -> bool {
        matches!(self, Bind::Pad(_))
    }

    /// Held now (the wheel is never held).
    pub fn held(self, input: &InputState) -> bool {
        match self {
            Bind::Key(c) => input.down(c),
            Bind::Mouse(b) => input.mouse_buttons & b != 0,
            Bind::Pad(b) => input.pad.held(b),
            Bind::WheelUp | Bind::WheelDown => false,
        }
    }

    /// Went down since the last frame.
    pub fn pressed(self, input: &InputState) -> bool {
        match self {
            Bind::Key(c) => input.pressed(c),
            Bind::Mouse(b) => input.mouse_pressed(b),
            Bind::Pad(b) => input.pad_pressed(b),
            Bind::WheelUp => input.wheel > 0,
            Bind::WheelDown => input.wheel < 0,
        }
    }

    pub fn name(self) -> String {
        match self {
            Bind::Key(c) => key_name(c).map_or_else(|| format!("KEY {c:02X}"), str::to_string),
            Bind::Mouse(b) => {
                MOUSE.iter().find(|(x, _)| *x == b).map_or("MOUSE ?", |(_, n)| n).to_string()
            }
            Bind::WheelUp => "WHEEL UP".into(),
            Bind::WheelDown => "WHEEL DOWN".into(),
            Bind::Pad(b) => {
                PAD.iter().find(|(x, _)| *x == b).map_or("PAD ?", |(_, n)| n).to_string()
            }
        }
    }

    /// A short name for the bindings screen (the screen font is wide).
    pub fn short_name(self) -> String {
        match self {
            Bind::Key(0x2A) => "LSHIFT".into(),
            Bind::Key(0x36) => "RSHIFT".into(),
            Bind::Key(0x45) => "NUMLOCK".into(),
            Bind::Key(0x46) => "SCRLOCK".into(),
            Bind::Key(0x49) => "PGUP".into(),
            Bind::Key(0x51) => "PGDN".into(),
            Bind::Key(0x3A) => "CAPS".into(),
            Bind::Key(0x0E) => "BKSP".into(),
            Bind::Key(0x52) => "INS".into(),
            Bind::Key(0x53) => "DEL".into(),
            Bind::Mouse(1) => "MOUSE1".into(),
            Bind::Mouse(4) => "MOUSE2".into(),
            Bind::Mouse(2) => "MOUSE3".into(),
            Bind::Mouse(8) => "MOUSE4".into(),
            Bind::Mouse(16) => "MOUSE5".into(),
            Bind::WheelUp => "WHEEL UP".into(),
            Bind::WheelDown => "WHEEL DN".into(),
            Bind::Pad(_) => self.name().trim_start_matches("PAD ").to_string(),
            b => b.name(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        match s {
            "WHEEL UP" => return Some(Bind::WheelUp),
            "WHEEL DOWN" => return Some(Bind::WheelDown),
            _ => {}
        }
        if let Some((b, _)) = MOUSE.iter().find(|(_, n)| *n == s) {
            return Some(Bind::Mouse(*b));
        }
        if let Some((b, _)) = PAD.iter().find(|(_, n)| *n == s) {
            return Some(Bind::Pad(*b));
        }
        if let Some(hex) = s.strip_prefix("KEY ") {
            return u8::from_str_radix(hex, 16).ok().map(Bind::Key);
        }
        (1..=0x7Fu8).find(|&c| key_name(c) == Some(s)).map(Bind::Key)
    }

    /// The input that went down this frame, if any: keys and mouse when
    /// `pad` is false, gamepad buttons when true. Used to rebind.
    pub fn captured(input: &InputState, pad: bool) -> Option<Self> {
        if pad {
            return PAD.iter().map(|(b, _)| Bind::Pad(*b)).find(|b| b.pressed(input));
        }
        if let Some(e) = input.events.iter().find(|e| e.down && !e.repeat) {
            return Some(Bind::Key(e.code));
        }
        MOUSE
            .iter()
            .map(|(b, _)| Bind::Mouse(*b))
            .chain([Bind::WheelUp, Bind::WheelDown])
            .find(|b| b.pressed(input))
    }
}

impl Serialize for Bind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.name())
    }
}

impl<'de> Deserialize<'de> for Bind {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Bind::parse(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown input {s:?}")))
    }
}

const MOUSE: [(u8, &str); 5] =
    [(1, "MOUSE LEFT"), (4, "MOUSE RIGHT"), (2, "MOUSE MIDDLE"), (8, "MOUSE 4"), (16, "MOUSE 5")];

const PAD: [(u32, &str); 16] = [
    (pad::SOUTH, "PAD A"),
    (pad::EAST, "PAD B"),
    (pad::WEST, "PAD X"),
    (pad::NORTH, "PAD Y"),
    (pad::LEFT_SHOULDER, "PAD LB"),
    (pad::RIGHT_SHOULDER, "PAD RB"),
    (pad::LEFT_TRIGGER, "PAD LT"),
    (pad::RIGHT_TRIGGER, "PAD RT"),
    (pad::BACK, "PAD BACK"),
    (pad::START, "PAD START"),
    (pad::LEFT_STICK, "PAD L3"),
    (pad::RIGHT_STICK, "PAD R3"),
    (pad::DPAD_UP, "DPAD UP"),
    (pad::DPAD_DOWN, "DPAD DOWN"),
    (pad::DPAD_LEFT, "DPAD LEFT"),
    (pad::DPAD_RIGHT, "DPAD RIGHT"),
];

/// Names of the PC set-1 scan codes the platform produces.
pub fn key_name(code: u8) -> Option<&'static str> {
    const NAMES: [&str; 0x59] = [
        "",
        "ESC",
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "9",
        "0",
        "-",
        "=",
        "BACKSPACE",
        "TAB",
        "Q",
        "W",
        "E",
        "R",
        "T",
        "Y",
        "U",
        "I",
        "O",
        "P",
        "[",
        "]",
        "ENTER",
        "CTRL",
        "A",
        "S",
        "D",
        "F",
        "G",
        "H",
        "J",
        "K",
        "L",
        ";",
        "'",
        "`",
        "LEFT SHIFT",
        "\\",
        "Z",
        "X",
        "C",
        "V",
        "B",
        "N",
        "M",
        ",",
        ".",
        "/",
        "RIGHT SHIFT",
        "KEYPAD *",
        "ALT",
        "SPACE",
        "CAPS LOCK",
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
        "F7",
        "F8",
        "F9",
        "F10",
        "NUM LOCK",
        "SCROLL LOCK",
        "HOME",
        "UP",
        "PAGE UP",
        "KEYPAD -",
        "LEFT",
        "KEYPAD 5",
        "RIGHT",
        "KEYPAD +",
        "END",
        "DOWN",
        "PAGE DOWN",
        "INSERT",
        "DELETE",
        "",
        "",
        "",
        "F11",
        "F12",
    ];
    match code {
        0x7F => Some("PAUSE"),
        c => NAMES.get(usize::from(c)).copied().filter(|n| !n.is_empty()),
    }
}

/// Every action's inputs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(transparent)]
pub struct Bindings(pub BTreeMap<Action, Vec<Bind>>);

/// Reading is lenient: an unknown action or input name (a typo in a hand-edited
/// `obcr.toml`, or a file from a newer version) is skipped instead of making the
/// whole settings file unreadable, which would reset every setting.
impl<'de> Deserialize<'de> for Bindings {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Entry {
            Names(Vec<String>),
            Other(serde::de::IgnoredAny),
        }
        let raw = BTreeMap::<String, Entry>::deserialize(d)?;
        let mut m = BTreeMap::new();
        for (action, entry) in raw {
            let action = Action::deserialize(serde::de::value::StrDeserializer::<
                serde::de::value::Error,
            >::new(&action));
            if let (Ok(action), Entry::Names(names)) = (action, entry) {
                m.insert(action, names.iter().filter_map(|n| Bind::parse(n)).collect());
            }
        }
        Ok(Self(m))
    }
}

impl Bindings {
    /// The defaults of a control scheme (`modern` or `original`).
    pub fn defaults(scheme: &str) -> Self {
        use Action as A;
        use Bind::{Key, Mouse, Pad};
        use obc_core::input::sc;
        const C: u8 = 0x2E;
        const F: u8 = 0x21;
        const V: u8 = 0x2F;
        const Z: u8 = 0x2C;
        const F2: u8 = 0x3C;
        const F5: u8 = 0x3F;
        const F6: u8 = 0x40;
        const F8: u8 = 0x42;
        let modern = scheme != "original";
        let mut m: BTreeMap<Action, Vec<Bind>> = BTreeMap::new();
        let mut set = |a: Action, b: &[Bind]| {
            m.insert(a, b.to_vec());
        };
        if modern {
            set(A::Forward, &[Key(sc::W), Key(sc::UP), Pad(pad::DPAD_UP)]);
            set(A::Back, &[Key(sc::S), Key(sc::DOWN)]);
            set(A::TurnLeft, &[Key(sc::LEFT), Pad(pad::DPAD_LEFT)]);
            set(A::TurnRight, &[Key(sc::RIGHT), Pad(pad::DPAD_RIGHT)]);
            set(A::StrafeLeft, &[Key(sc::A)]);
            set(A::StrafeRight, &[Key(sc::D)]);
            set(A::Strafe, &[]);
            set(
                A::Run,
                &[Key(sc::LSHIFT), Key(sc::RSHIFT), Pad(pad::LEFT_TRIGGER), Pad(pad::LEFT_STICK)],
            );
            set(A::Fire, &[Key(sc::CONTROL), Mouse(1), Pad(pad::RIGHT_TRIGGER)]);
            set(A::Use, &[Key(sc::SPACE), Key(sc::E), Mouse(4), Pad(pad::SOUTH)]);
            set(A::NextWeapon, &[Bind::WheelUp, Pad(pad::RIGHT_SHOULDER)]);
            set(A::PrevWeapon, &[Bind::WheelDown, Pad(pad::LEFT_SHOULDER)]);
            set(A::AllySwitch, &[Key(sc::Q), Pad(pad::WEST)]);
            set(A::AllyStay, &[Key(Z), Pad(pad::EAST)]);
            set(A::WipeVisor, &[Key(V), Pad(pad::DPAD_DOWN)]);
        } else {
            set(A::Forward, &[Key(sc::UP), Pad(pad::DPAD_UP)]);
            set(A::Back, &[Key(sc::DOWN)]);
            set(A::TurnLeft, &[Key(sc::LEFT), Pad(pad::DPAD_LEFT)]);
            set(A::TurnRight, &[Key(sc::RIGHT), Pad(pad::DPAD_RIGHT)]);
            set(A::StrafeLeft, &[]);
            set(A::StrafeRight, &[]);
            set(A::Strafe, &[Key(sc::ALT)]);
            // The original's mouse: right button runs (and walks, controls.rs),
            // middle uses.
            set(
                A::Run,
                &[
                    Key(sc::LSHIFT),
                    Key(sc::RSHIFT),
                    Mouse(4),
                    Pad(pad::LEFT_TRIGGER),
                    Pad(pad::LEFT_STICK),
                ],
            );
            set(A::Fire, &[Key(sc::CONTROL), Mouse(1), Pad(pad::RIGHT_TRIGGER)]);
            set(A::Use, &[Key(sc::SPACE), Mouse(2), Pad(pad::SOUTH)]);
            set(A::NextWeapon, &[Bind::WheelUp, Pad(pad::RIGHT_SHOULDER)]);
            set(A::PrevWeapon, &[Bind::WheelDown, Pad(pad::LEFT_SHOULDER)]);
            set(A::AllySwitch, &[Key(sc::A), Pad(pad::WEST)]);
            set(A::AllyStay, &[Key(sc::S), Pad(pad::EAST)]);
            set(A::WipeVisor, &[Key(sc::W), Pad(pad::DPAD_DOWN)]);
        }
        for (i, a) in
            [A::Weapon1, A::Weapon2, A::Weapon3, A::Weapon4, A::Weapon5].into_iter().enumerate()
        {
            set(a, &[Key(sc::DIGIT_1 + i as u8)]);
        }
        set(A::AllyMode, &[Key(F), Pad(pad::NORTH)]);
        set(A::Map, &[Key(sc::TAB), Pad(pad::BACK)]);
        set(A::Crosshair, &[Key(C)]);
        set(A::Health, &[Key(sc::H)]);
        set(A::Tremors, &[Key(F8)]);
        set(A::Pause, &[Key(sc::PAUSE)]);
        set(A::QuickSave, &[Key(sc::F9)]);
        set(A::SoundToggle, &[Key(sc::F1)]);
        set(A::MusicToggle, &[Key(F2)]);
        // CheckKeys (1942:0666, 1942:0688): F5 floormode 0 (off), F6 2 (on).
        set(A::FloorsOff, &[Key(F5)]);
        set(A::FloorsOn, &[Key(F6)]);
        set(A::Menu, &[Key(sc::ESCAPE), Pad(pad::START)]);
        set(A::Quit, &[Key(sc::F10)]);
        Self(m)
    }

    /// Fills actions missing from a saved file (new actions, old files) from
    /// the scheme's defaults.
    pub fn complete(&mut self, scheme: &str) {
        for (a, b) in Self::defaults(scheme).0 {
            self.0.entry(a).or_insert(b);
        }
    }

    pub fn get(&self, a: Action) -> &[Bind] {
        self.0.get(&a).map_or(&[], Vec::as_slice)
    }

    pub fn held(&self, a: Action, input: &InputState) -> bool {
        self.get(a).iter().any(|b| b.held(input))
    }

    pub fn pressed(&self, a: Action, input: &InputState) -> bool {
        self.get(a).iter().any(|b| b.pressed(input))
    }

    /// Binds `b` to `a`, taking it from any other action. Keyboard/mouse and
    /// gamepad inputs are capped separately; the oldest one goes first.
    /// Returns the actions `b` was taken from.
    pub fn bind(&mut self, a: Action, b: Bind) -> Vec<Action> {
        let mut taken = Vec::new();
        for (other, list) in self.0.iter_mut() {
            if *other != a && list.contains(&b) {
                list.retain(|x| *x != b);
                taken.push(*other);
            }
        }
        let list = self.0.entry(a).or_default();
        if list.contains(&b) {
            return taken;
        }
        let cap = if b.is_pad() { MAX_PAD } else { MAX_KEYS };
        while list.iter().filter(|x| x.is_pad() == b.is_pad()).count() >= cap {
            if let Some(i) = list.iter().position(|x| x.is_pad() == b.is_pad()) {
                list.remove(i);
            }
        }
        list.push(b);
        taken
    }

    /// Removes the keyboard/mouse (`pad` false) or gamepad inputs of `a`.
    pub fn clear(&mut self, a: Action, pad: bool) {
        if let Some(list) = self.0.get_mut(&a) {
            list.retain(|b| b.is_pad() != pad);
        }
    }

    /// The inputs of `a` as text, keyboard/mouse or gamepad.
    pub fn describe(&self, a: Action, pad: bool) -> String {
        let names: Vec<String> =
            self.get(a).iter().filter(|b| b.is_pad() == pad).map(|b| b.short_name()).collect();
        if names.is_empty() { "-".into() } else { names.join(", ") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for scheme in ["modern", "original"] {
            for list in Bindings::defaults(scheme).0.values() {
                for b in list {
                    assert_eq!(Bind::parse(&b.name()), Some(*b), "{}", b.name());
                }
            }
        }
        for c in 1..=0x58u8 {
            if let Some(n) = key_name(c) {
                assert_eq!(Bind::parse(n), Some(Bind::Key(c)), "{n}");
            }
        }
        assert_eq!(Bind::parse("KEY 5A"), Some(Bind::Key(0x5A)));
    }

    #[test]
    fn every_action_has_a_default() {
        for scheme in ["modern", "original"] {
            let d = Bindings::defaults(scheme);
            for (a, _) in ACTIONS {
                assert!(d.0.contains_key(&a), "{scheme}: {a:?}");
            }
        }
    }

    #[test]
    fn binding_moves_an_input_and_caps_slots() {
        let mut b = Bindings::defaults("modern");
        let taken = b.bind(Action::Map, Bind::Key(0x11)); // W was Forward
        assert_eq!(taken, vec![Action::Forward]);
        assert!(!b.get(Action::Forward).contains(&Bind::Key(0x11)));
        assert!(b.get(Action::Map).contains(&Bind::Key(0x11)));
        // Use has three key/mouse inputs: a fourth replaces the oldest.
        b.bind(Action::Use, Bind::Key(0x22));
        let keys: Vec<_> = b.get(Action::Use).iter().filter(|x| !x.is_pad()).collect();
        assert_eq!(keys.len(), MAX_KEYS);
        assert!(!keys.contains(&&Bind::Key(0x39)), "SPACE was the oldest");
        assert!(b.get(Action::Use).contains(&Bind::Pad(pad::SOUTH)), "gamepad untouched");
        b.clear(Action::Use, false);
        assert_eq!(b.get(Action::Use), &[Bind::Pad(pad::SOUTH)]);
    }

    #[test]
    fn saved_as_names() {
        #[derive(Serialize, Deserialize)]
        struct W {
            bindings: Bindings,
        }
        let text = toml::to_string(&W { bindings: Bindings::defaults("modern") }).unwrap();
        assert!(text.contains("fire = [\"CTRL\", \"MOUSE LEFT\", \"PAD RT\"]"), "{text}");
        let back: W = toml::from_str(&text).unwrap();
        assert_eq!(back.bindings, Bindings::defaults("modern"));
    }

    #[test]
    fn unknown_names_are_skipped_not_fatal() {
        #[derive(Deserialize)]
        struct W {
            bindings: Bindings,
        }
        let w: W = toml::from_str(
            "[bindings]
fire = [\"CTRL\", \"MOUSE 9\", \"PAD RT\"]
teleport = [\"T\"]
use = 3
",
        )
        .unwrap();
        assert_eq!(
            w.bindings.get(Action::Fire),
            &[Bind::Key(obc_core::input::sc::CONTROL), Bind::Pad(pad::RIGHT_TRIGGER)]
        );
        assert_eq!(w.bindings.0.len(), 1, "unknown action and wrong type dropped");
    }
}
