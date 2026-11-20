//! Opt-in fixes for bugs of the original game (DESIGN §4.3, ADR-008).
//!
//! Classic plays every original bug as the original does. A gameplay bug may
//! get a fix the player turns on in OPTIONS > GAMEPLAY FIXES; each is a quirk
//! `K-###` in docs/re/DELTA_SPEC.md, and its bit is its quirk number. The
//! fixes in force are part of the world (set when a game starts, kept in its
//! saves) and of the network identity, so two machines never disagree.
//!
//! Code that fixes a quirk asks `w.fixes.on(&fixes::K_0NN)` and keeps the
//! original's path otherwise.

use serde::{Deserialize, Serialize};

/// One fixable quirk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fix {
    /// Its quirk number (`K-002` is 2): the bit it uses.
    pub number: u32,
    /// Shown in the menu (upper case; the game font).
    pub label: &'static str,
    /// One line on what the fix changes (upper case, at most 36 characters).
    pub about: &'static str,
}

impl Fix {
    pub fn id(&self) -> String {
        format!("K-{:03}", self.number)
    }

    fn mask(&self) -> u64 {
        1u64 << self.number.min(63)
    }
}

/// The fixes in force: a bit per quirk number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fixes(pub u64);

impl Fixes {
    /// None: the original game.
    pub const ORIGINAL: Self = Self(0);

    pub fn on(self, f: &Fix) -> bool {
        self.0 & f.mask() != 0
    }

    pub fn set(&mut self, f: &Fix, on: bool) {
        if on {
            self.0 |= f.mask();
        } else {
            self.0 &= !f.mask();
        }
    }

    /// Only bits of fixes this version knows (settings or saves written by
    /// another version may carry others).
    pub fn known(self) -> Self {
        Self(self.0 & ALL.iter().fold(0, |m, f| m | f.mask()))
    }
}

/// K-001: `PlayLoop` clears the Map ThermoSensor on every entry, a load
/// included, and the original's save does not hold it (docs/re/DELTA_SPEC.md).
/// The fix keeps it across a load; a new floor still starts without it.
pub const K_001: Fix =
    Fix { number: 1, label: "KEEP HEAT SENSOR ON LOAD", about: "A LOADED GAME KEEPS THE SENSOR" };

/// K-004: an opened door-look wall (an animated wall in its open state)
/// blocks `CheckLine`, so shots and enemy sight stop at it although the
/// player walks through (docs/re/DELTA_SPEC.md; GitHub issue #12).
pub const K_004: Fix = Fix {
    number: 4,
    label: "SEE THROUGH OPEN DOOR WALLS",
    about: "SHOTS AND SIGHT PASS OPEN WALL DOORS",
};

/// K-002: items are picked up only when they are ahead of the view
/// (docs/re/DELTA_SPEC.md); the fix also picks up an item in the player's
/// own cell, whichever way the player moves.
pub const K_002: Fix = Fix {
    number: 2,
    label: "PICK UP ITEMS UNDERFOOT",
    about: "ALSO WHEN STRAFING OR BACKING ONTO",
};

/// K-003: picking up a weapon selects it whenever it is at least the best
/// weapon, even one already owned; the fix selects only a newly owned one.
pub const K_003: Fix =
    Fix { number: 3, label: "KEEP WEAPON ON PICKUP", about: "SWITCH ONLY TO A NEW WEAPON" };

/// Every fix, in menu order.
pub const ALL: &[&Fix] = &[&K_001, &K_002, &K_003, &K_004];

#[cfg(test)]
mod tests {
    use super::*;

    const T: Fix = Fix { number: 7, label: "TEST", about: "TEST" };

    #[test]
    fn bits_follow_quirk_numbers() {
        let mut f = Fixes::ORIGINAL;
        assert!(!f.on(&T));
        f.set(&T, true);
        assert_eq!((f.0, T.id()), (1 << 7, "K-007".to_owned()));
        f.set(&T, false);
        assert_eq!(f, Fixes::ORIGINAL);
    }

    #[test]
    fn quirk_numbers_are_unique() {
        for (i, a) in ALL.iter().enumerate() {
            assert!(a.number < 64, "{}", a.id());
            assert!(ALL[i + 1..].iter().all(|b| b.number != a.number), "{}", a.id());
        }
    }
}
