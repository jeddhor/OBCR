//! Events from the simulation to presentation (DESIGN §7.2.5).

/// Something the presentation should react to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimEvent {
    /// `SD_PlaySound(id)` heard everywhere.
    Sound(u16),
    /// `PlaySoundAt(id, x, y)`: positional sound at a world position (16.16).
    SoundAt { id: u16, x: i32, y: i32 },
    /// Damage flash of the given strength (`StartDamageFlash`).
    DamageFlash(i16),
    /// Bonus flash (`StartBonusFlash`).
    BonusFlash,
    /// In-game message (`GameMessage`), shown for 125 tics.
    Message(String),
    /// Item icon in the event box.
    ItemIcon(u16),
    /// Ally death animation in the event box.
    AllyDied,
    /// Screen shake for this many frames.
    Shake(i16),
    /// Screen splat (`AddScreenSplat`).
    Splat(u8),
    /// Status bar needs redrawing.
    Hud,
    /// The original fades to black on a heavy low-health hit.
    Blackout,
    /// Gamepad rumble request (Enhanced): strength 0..1 scaled ×100.
    Rumble(u8),
    /// Lighting mode change (`2f4d:035e`).
    Lighting(u8),
    /// An invariant was violated; logged, the simulation continues.
    Fault(String),
}
