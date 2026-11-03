//! Device input → [`TicCmd`] (`PollControls` 1942:0400, docs/re/PLAYER.md §3.1–3.2).
//!
//! Input arrives every 70 Hz tic; a game frame runs every two (ADR-013). Key
//! presses between frames are latched so a quick tap is never lost, and mouse
//! motion is summed.
//!
//! Profiles:
//! - **original**: the DOS defaults — arrows, Ctrl fire, Alt strafe, Shift
//!   run, Space use, 1–5 weapons, A/S/F ally keys, W wipe, F8 tremors; the
//!   mouse turns (`mx·10/(13−5)`), left fires, right runs (and walks forward,
//!   as the original's mouse code does), middle uses.
//! - **modern**: W/S move, A/D strafe while turning, mouse look, LMB fire,
//!   RMB/Space/E use, Shift run (inverted by "always run"), wheel or 1–5
//!   weapons, Q/Z/F ally keys, V wipe.
//! - **gamepad** (always on): left stick move/strafe, right stick turn, RT
//!   fire, A use, LB/RB weapon cycle, X ally switch, B stay & switch,
//!   Y ally mode, D-pad down wipe, Back map, LT/L3 run.

use obc_core::input::{InputState, pad, sc};
use obc_sim::world::{NUMBUTTONS, TicCmd, World, bt};

use crate::settings::InputSettings;

const BASEMOVE: i32 = 35;
const RUNMOVE: i32 = 70;

/// Scan codes not in `obc_core::input::sc`.
mod key {
    pub const C: u8 = 0x2E;
    pub const F: u8 = 0x21;
    pub const V: u8 = 0x2F;
    pub const Z: u8 = 0x2C;
    pub const F5: u8 = 0x3F;
    pub const F6: u8 = 0x40;
    pub const F8: u8 = 0x42;
}

/// Presentation-side keys (not part of the simulation).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UiKeys {
    pub menu: bool,
    pub pause: bool,
    pub floor_on: bool,
    pub floor_off: bool,
    pub health_overlay: bool,
    pub crosshair: bool,
    pub quicksave: bool,
}

#[derive(Debug, Default)]
pub struct Controls {
    mouse_dx: f32,
    /// Buttons pressed since the last frame (held or not now).
    latched: [bool; NUMBUTTONS],
    ally_switch: bool,
    ally_stay: bool,
    ally_mode: bool,
    tremors: bool,
    wipe: bool,
    weapon_step: i8,
    ui: UiKeys,
}

impl Controls {
    /// Call every tic with that tic's input.
    pub fn accumulate(&mut self, input: &InputState, s: &InputSettings) {
        let modern = s.profile != "original";
        self.mouse_dx += input.mouse_dx;
        let (now, _) = self.buttons(input, s);
        for (l, n) in self.latched.iter_mut().zip(now) {
            *l |= n;
        }
        let p = |c| input.pressed(c);
        let pp = |b| input.pad_pressed(b);
        if modern {
            self.ally_switch |= p(sc::Q) || pp(pad::WEST);
            self.ally_stay |= p(key::Z) || pp(pad::EAST);
            self.ally_mode |= p(key::F) || pp(pad::NORTH);
            self.wipe |= p(key::V) || pp(pad::DPAD_DOWN);
        } else {
            self.ally_switch |= p(sc::A) || pp(pad::WEST);
            self.ally_stay |= p(sc::S) || pp(pad::EAST);
            self.ally_mode |= p(key::F) || pp(pad::NORTH);
            self.wipe |= p(sc::W) || pp(pad::DPAD_DOWN);
        }
        self.tremors |= p(key::F8);
        if pp(pad::RIGHT_SHOULDER) {
            self.weapon_step = 1;
        }
        if pp(pad::LEFT_SHOULDER) {
            self.weapon_step = -1;
        }
        // Mouse wheel: away from you selects the next weapon.
        if input.wheel != 0 {
            self.weapon_step = if input.wheel > 0 { 1 } else { -1 };
        }
        let ui = &mut self.ui;
        ui.menu |= p(sc::ESCAPE) || pp(pad::START);
        ui.pause |= p(sc::PAUSE);
        ui.floor_on |= p(key::F5);
        ui.floor_off |= p(key::F6);
        ui.health_overlay |= p(sc::H);
        ui.crosshair |= p(key::C);
        ui.quicksave |= p(sc::F9);
    }

    /// Held buttons now, and whether run is active.
    fn buttons(&self, input: &InputState, s: &InputSettings) -> ([bool; NUMBUTTONS], bool) {
        let modern = s.profile != "original";
        let d = |c| input.down(c);
        let m = input.mouse_buttons;
        let pad = &input.pad;
        let mut b = [false; NUMBUTTONS];
        if modern {
            b[bt::ATTACK] = d(sc::CONTROL) || m & 1 != 0;
            b[bt::USE] = d(sc::SPACE) || d(sc::E) || m & 4 != 0;
            let shift = d(sc::LSHIFT) || d(sc::RSHIFT);
            b[bt::RUN] = shift != s.always_run;
            b[bt::MAP] = d(sc::TAB);
            b[bt::TOGGLE_I] = d(sc::I);
        } else {
            b[bt::ATTACK] = d(sc::CONTROL) || m & 1 != 0;
            b[bt::RUN] = d(sc::RSHIFT) || d(sc::LSHIFT) || m & 4 != 0 || s.always_run;
            b[bt::USE] = d(sc::SPACE) || m & 2 != 0;
            b[bt::STRAFE] = d(sc::ALT);
            b[bt::MAP] = d(sc::TAB);
            b[bt::TOGGLE_S] = d(sc::S);
            b[bt::TOGGLE_I] = d(sc::I);
        }
        for i in 0..5 {
            b[bt::WEAPON1 + i] |= d(sc::DIGIT_1 + i as u8);
        }
        b[bt::ATTACK] |= pad.right_trigger > 0.5;
        b[bt::USE] |= pad.held(pad::SOUTH);
        b[bt::RUN] |= pad.left_trigger > 0.5 || pad.held(pad::LEFT_STICK);
        b[bt::MAP] |= pad.held(pad::BACK);
        (b, b[bt::RUN])
    }

    /// Builds the frame's command and the UI keys, then clears the latches.
    pub fn build(
        &mut self,
        input: &InputState,
        s: &InputSettings,
        w: &World,
        tics: i16,
    ) -> (TicCmd, UiKeys) {
        let modern = s.profile != "original";
        let (held, run) = self.buttons(input, s);
        let mut cmd = TicCmd::default();
        for ((b, h), l) in cmd.buttons.iter_mut().zip(held).zip(self.latched) {
            *b = h || l;
        }
        let t = i32::from(tics);
        let mv = if run { RUNMOVE } else { BASEMOVE } * t;
        let d = |c| input.down(c);
        let (mut cx, mut cy, mut strafe, mut turn) = (0i32, 0i32, 0i32, 0i32);
        if modern {
            if d(sc::W) || d(sc::UP) {
                cy -= mv;
            }
            if d(sc::S) || d(sc::DOWN) {
                cy += mv;
            }
            if d(sc::A) {
                strafe -= mv;
            }
            if d(sc::D) {
                strafe += mv;
            }
            if d(sc::LEFT) {
                cx -= mv;
            }
            if d(sc::RIGHT) {
                cx += mv;
            }
            // About 0.15° per pixel at sensitivity 1.
            turn += (self.mouse_dx * 3.0 * s.mouse_sensitivity) as i32;
        } else {
            if d(sc::UP) {
                cy -= mv;
            }
            if d(sc::DOWN) {
                cy += mv;
            }
            if d(sc::LEFT) {
                cx -= mv;
            }
            if d(sc::RIGHT) {
                cx += mv;
            }
            // PollMouseMove: mx·10/(13 − mouseadjustment), adjustment 5 by default.
            cx += (self.mouse_dx * 10.0 / 8.0 * s.mouse_sensitivity) as i32;
            if input.mouse_buttons & 4 != 0 {
                cy -= BASEMOVE * t;
            }
        }

        // Gamepad: full deflection runs; turning up to ~200°/s × turn speed.
        let pad = &input.pad;
        if pad.connected {
            let dz = s.gamepad_deadzone;
            let axis = |v: f32| {
                if v.abs() < dz { 0.0 } else { (v.abs() - dz) / (1.0 - dz) * v.signum() }
            };
            let speed = RUNMOVE as f32 * t as f32;
            cy += (axis(pad.left_y) * speed) as i32;
            strafe += (axis(pad.left_x) * speed) as i32;
            let rx = axis(pad.right_x);
            turn += (rx * rx.abs() * 57.0 * t as f32 * s.gamepad_turn_speed) as i32;
            if pad.held(pad::DPAD_UP) {
                cy -= mv;
            }
            if pad.held(pad::DPAD_LEFT) {
                cx -= mv;
            }
            if pad.held(pad::DPAD_RIGHT) {
                cx += mv;
            }
        }

        cmd.controlx = cx.clamp(-32000, 32000) as i16;
        cmd.controly = cy.clamp(-32000, 32000) as i16;
        cmd.strafe = strafe.clamp(-32000, 32000) as i16;
        cmd.turn = turn.clamp(-32000, 32000) as i16;
        cmd.ally_switch = self.ally_switch;
        cmd.ally_stay = self.ally_stay;
        cmd.ally_mode = self.ally_mode;
        cmd.toggle_tremors = self.tremors;
        cmd.wipe_visor = self.wipe;
        if self.weapon_step != 0
            && let Some(next) = cycle_weapon(w, self.weapon_step)
        {
            cmd.buttons[bt::WEAPON1 + next] = true;
        }
        let ui = self.ui;
        *self = Self::default();
        (cmd, ui)
    }
}

/// Next owned weapon after the chosen one in `step` direction.
fn cycle_weapon(w: &World, step: i8) -> Option<usize> {
    let g = &w.gamestate;
    let cur = g.chosenweapon.clamp(0, 4) as i32;
    (1..5).map(|i| (cur + i * i32::from(step)).rem_euclid(5) as usize).find(|&i| {
        g.weapons & (1 << i) != 0 && (i == 0 || g.clips[i - 1] > 0 || i == g.weapon as usize)
    })
}
