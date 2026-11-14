//! Device input → [`TicCmd`] (`PollControls` 1942:0400, docs/re/PLAYER.md §3.1–3.2).
//!
//! Input arrives every 70 Hz tic; a game frame runs every two (ADR-013). Key
//! presses between frames are latched so a quick tap is never lost, and mouse
//! motion is summed.
//!
//! What each key, mouse button and gamepad button does comes from the
//! player's [`Bindings`](crate::bindings::Bindings) (issue #2). The control
//! scheme decides the rest:
//! - **original**: the mouse turns as `PollMouseMove` does (`mx·10/(13−5)`),
//!   "always run" just runs, and a mouse button bound to RUN also walks
//!   forward at walking speed, as the original's mouse code does.
//! - **modern**: the mouse turns freely (mouse look), and "always run"
//!   inverts the run input.
//!
//! Always, whatever the bindings: the left stick moves and strafes, the right
//! stick turns, Esc and the gamepad's Start open the menu, Alt+letter are the
//! bogus-mode keys, and I (and S in the original scheme) are the original's
//! option-bit keys.

use obc_core::input::{InputState, pad, sc};
use obc_sim::world::{Cheat, NUMBUTTONS, TicCmd, World, bt};

use crate::bindings::{Action, Bind};
use crate::settings::InputSettings;

const BASEMOVE: i32 = 35;
const RUNMOVE: i32 = 70;

/// Scan codes of the bogus-mode keys not in `obc_core::input::sc`.
mod key {
    pub const B: u8 = 0x30;
    pub const R: u8 = 0x13;
    pub const P: u8 = 0x19;
    pub const S: u8 = 0x1F;
    pub const F: u8 = 0x21;
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
    pub sound_toggle: bool,
    pub music_toggle: bool,
    /// Alt+W in bogus mode: open the ` FLOOR: ` prompt.
    pub warp: bool,
    /// The `BACK TO DOS` prompt.
    pub quit: bool,
    /// CD release, Alt+P / Alt+S / Alt+F: play, stop, next track
    /// (CheckKeys cd:19cd:0003, CD_RELEASE.md §3.9).
    pub cd_play: bool,
    pub cd_stop: bool,
    pub cd_next: bool,
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
    cheat: Cheat,
    ui: UiKeys,
}

impl Controls {
    /// Call every tic with that tic's input.
    /// `nsr250`: the cheat parameter was given (Alt+LShift+B allowed).
    pub fn accumulate(&mut self, input: &InputState, s: &InputSettings, nsr250: bool) {
        self.mouse_dx += input.mouse_dx;
        let (now, _) = buttons(input, s);
        for (l, n) in self.latched.iter_mut().zip(now) {
            *l |= n;
        }
        let p = |c| input.pressed(c);
        // Alt combinations are the bogus-mode keys (GAMEFLOW §11.1); while Alt
        // is down the plain letter keys do nothing else.
        if input.down(sc::ALT) {
            if input.down(sc::LSHIFT) {
                if p(key::B) && nsr250 {
                    self.cheat = Cheat::Bogus;
                }
            } else if p(key::R) {
                self.cheat = Cheat::Restock;
            } else if p(sc::I) {
                self.cheat = Cheat::Invincible;
            } else if p(sc::H) {
                self.cheat = Cheat::HealthHelp;
            } else if p(key::B) {
                self.cheat = Cheat::Bewitched;
            } else if p(sc::W) {
                self.ui.warp = true;
            } else if p(key::P) {
                self.ui.cd_play = true;
            } else if p(key::S) {
                self.ui.cd_stop = true;
            } else if p(key::F) {
                self.ui.cd_next = true;
            }
            return;
        }
        let b = &s.bindings;
        let on = |a| b.pressed(a, input);
        self.ally_switch |= on(Action::AllySwitch);
        self.ally_stay |= on(Action::AllyStay);
        self.ally_mode |= on(Action::AllyMode);
        self.wipe |= on(Action::WipeVisor);
        self.tremors |= on(Action::Tremors);
        if on(Action::NextWeapon) {
            self.weapon_step = 1;
        }
        if on(Action::PrevWeapon) {
            self.weapon_step = -1;
        }
        let ui = &mut self.ui;
        ui.menu |= on(Action::Menu) || p(sc::ESCAPE) || input.pad_pressed(pad::START);
        ui.pause |= on(Action::Pause);
        ui.floor_on |= on(Action::FloorsOn);
        ui.floor_off |= on(Action::FloorsOff);
        ui.health_overlay |= on(Action::Health);
        ui.crosshair |= on(Action::Crosshair);
        ui.quicksave |= on(Action::QuickSave);
        ui.sound_toggle |= on(Action::SoundToggle);
        ui.music_toggle |= on(Action::MusicToggle);
        ui.quit |= on(Action::Quit);
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
        let b = &s.bindings;
        let (held, run) = buttons(input, s);
        let mut cmd = TicCmd::default();
        for ((c, h), l) in cmd.buttons.iter_mut().zip(held).zip(self.latched) {
            *c = h || l;
        }
        let t = i32::from(tics);
        let mv = if run { RUNMOVE } else { BASEMOVE } * t;
        let on = |a| b.held(a, input);
        let (mut cx, mut cy, mut strafe, mut turn) = (0i32, 0i32, 0i32, 0i32);
        if on(Action::Forward) {
            cy -= mv;
        }
        if on(Action::Back) {
            cy += mv;
        }
        if on(Action::TurnLeft) {
            cx -= mv;
        }
        if on(Action::TurnRight) {
            cx += mv;
        }
        if on(Action::StrafeLeft) {
            strafe -= mv;
        }
        if on(Action::StrafeRight) {
            strafe += mv;
        }
        if modern {
            // About 0.15° per pixel at sensitivity 1.
            turn += (self.mouse_dx * 3.0 * s.mouse_sensitivity) as i32;
        } else {
            // PollMouseMove: mx·10/(13 − mouseadjustment), adjustment 5 by default.
            cx += (self.mouse_dx * 10.0 / 8.0 * s.mouse_sensitivity) as i32;
            let mouse_run =
                b.get(Action::Run).iter().any(|x| matches!(x, Bind::Mouse(_)) && x.held(input));
            if mouse_run {
                cy -= BASEMOVE * t;
            }
        }

        // Gamepad sticks: full deflection runs; turning up to ~200°/s × turn speed.
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
        cmd.cheat = self.cheat;
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

/// Held buttons now, and whether run is active.
fn buttons(input: &InputState, s: &InputSettings) -> ([bool; NUMBUTTONS], bool) {
    let modern = s.profile != "original";
    let on = |a| s.bindings.held(a, input);
    let mut b = [false; NUMBUTTONS];
    b[bt::ATTACK] = on(Action::Fire);
    b[bt::USE] = on(Action::Use);
    b[bt::RUN] =
        if modern { on(Action::Run) != s.always_run } else { on(Action::Run) || s.always_run };
    b[bt::STRAFE] = on(Action::Strafe);
    b[bt::MAP] = on(Action::Map);
    // The original's option-bit keys (2f4d:0bd3), not rebindable.
    b[bt::TOGGLE_I] = input.down(sc::I);
    if !modern {
        b[bt::TOGGLE_S] = input.down(sc::S);
    }
    let weapons =
        [Action::Weapon1, Action::Weapon2, Action::Weapon3, Action::Weapon4, Action::Weapon5];
    for (i, a) in weapons.into_iter().enumerate() {
        b[bt::WEAPON1 + i] = on(a);
    }
    (b, b[bt::RUN])
}

/// Next owned weapon after the chosen one in `step` direction.
fn cycle_weapon(w: &World, step: i8) -> Option<usize> {
    let g = &w.gamestate;
    let cur = g.chosenweapon.clamp(0, 4) as i32;
    (1..5).map(|i| (cur + i * i32::from(step)).rem_euclid(5) as usize).find(|&i| {
        g.weapons & (1 << i) != 0 && (i == 0 || g.clips[i - 1] > 0 || i == g.weapon as usize)
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use obc_core::trig::Tables;
    use obc_sim::data::GameData;

    use super::*;
    use crate::bindings::Bindings;

    fn world() -> World {
        World::new(Arc::new(GameData::default()), Arc::new(Tables::build()))
    }

    fn frame(input: &InputState, s: &InputSettings) -> (TicCmd, UiKeys) {
        let mut c = Controls::default();
        c.accumulate(input, s, false);
        c.build(input, s, &world(), 2)
    }

    #[test]
    fn defaults_move_and_fire() {
        let s = InputSettings::default();
        let mut i = InputState::default();
        i.key(sc::W, true, false);
        i.key(sc::CONTROL, true, false);
        let (cmd, _) = frame(&i, &s);
        assert!(cmd.controly < 0, "W walks forward");
        assert!(cmd.buttons[bt::ATTACK], "Ctrl fires");
    }

    #[test]
    fn rebound_keys_follow_the_bindings() {
        let mut s = InputSettings::default();
        s.bindings.bind(Action::Fire, Bind::Key(0x21)); // F, taken from ALLY FOLLOW/ROAM
        s.bindings.clear(Action::Fire, false);
        s.bindings.bind(Action::Fire, Bind::Key(0x21));
        s.bindings.bind(Action::Crosshair, Bind::Mouse(8));
        let mut i = InputState::default();
        i.key(sc::CONTROL, true, false);
        let (cmd, _) = frame(&i, &s);
        assert!(!cmd.buttons[bt::ATTACK], "Ctrl no longer fires");
        let mut i = InputState::default();
        i.key(0x21, true, false);
        i.mouse_button(4, true); // side button
        let (cmd, ui) = frame(&i, &s);
        assert!(cmd.buttons[bt::ATTACK], "F fires");
        assert!(!cmd.ally_mode, "F no longer changes the ally mode");
        assert!(ui.crosshair, "mouse 4 toggles the crosshair");
    }

    #[test]
    fn original_mouse_run_also_walks() {
        let s = InputSettings {
            profile: "original".into(),
            bindings: Bindings::defaults("original"),
            ..InputSettings::default()
        };
        let mut i = InputState::default();
        i.mouse_button(3, true); // right
        let (cmd, _) = frame(&i, &s);
        assert!(cmd.buttons[bt::RUN]);
        assert_eq!(i32::from(cmd.controly), -BASEMOVE * 2, "walking speed, as the original");
    }

    #[test]
    fn escape_always_opens_the_menu() {
        let mut s = InputSettings::default();
        s.bindings.clear(Action::Menu, false);
        s.bindings.clear(Action::Menu, true);
        let mut i = InputState::default();
        i.key(sc::ESCAPE, true, false);
        assert!(frame(&i, &s).1.menu);
    }
}
