//! User settings persisted in `obcr.toml` (DESIGN §7.7).

use serde::{Deserialize, Serialize};

use crate::present::{Aspect, Presentation};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub input: InputSettings,
    pub game: GameSettings,
    pub data_dir: Option<String>,
}

/// Options the original kept in `CONFIG.DAT` (GAMEFLOW §10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GameSettings {
    /// GAME TYPE: 0–2 difficulty 1–3, 3 Random.
    pub gametype: i16,
    /// VIEW SIZE: 12, 14, 16, 18 or 20.
    pub viewsize: u16,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self { gametype: 1, viewsize: 18 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    /// "classic" or "enhanced".
    pub mode: String,
    /// Enhanced resolution multiple (2..=8).
    pub scale: usize,
    /// "4:3", "16:10", "16:9", "21:9".
    pub aspect: String,
    pub fullscreen: bool,
    pub vsync: bool,
    pub smooth: bool,
    pub integer_scale: bool,
    pub window_width: u32,
    pub window_height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub master: f32,
    pub sfx: f32,
    pub music: f32,
    /// Digitized voices: 1 as the original (one Sound Blaster voice with
    /// priorities), up to 4 mixed.
    pub channels: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputSettings {
    /// "modern" (WASD + mouse) or "original" (arrows).
    pub profile: String,
    pub mouse_sensitivity: f32,
    pub invert_mouse: bool,
    pub always_run: bool,
    pub gamepad_deadzone: f32,
    pub gamepad_turn_speed: f32,
    pub rumble: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            mode: "enhanced".into(),
            scale: 4,
            aspect: "16:9".into(),
            fullscreen: false,
            vsync: true,
            smooth: false,
            integer_scale: false,
            window_width: 1280,
            window_height: 720,
        }
    }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { master: 0.8, sfx: 1.0, music: 0.7, channels: 4 }
    }
}

impl Default for InputSettings {
    fn default() -> Self {
        Self {
            profile: "modern".into(),
            mouse_sensitivity: 1.0,
            invert_mouse: false,
            always_run: false,
            gamepad_deadzone: 0.2,
            gamepad_turn_speed: 1.0,
            rumble: true,
        }
    }
}

impl VideoSettings {
    pub fn presentation(&self) -> Presentation {
        if self.mode == "classic" {
            return Presentation::CLASSIC;
        }
        let aspect = match self.aspect.as_str() {
            "4:3" => Aspect::Standard,
            "16:10" => Aspect::Wide16x10,
            "21:9" => Aspect::Wide21x9,
            _ => Aspect::Wide16x9,
        };
        Presentation { scale: self.scale.clamp(1, 8), aspect }
    }

    pub fn set_presentation(&mut self, p: Presentation) {
        if p.is_classic() {
            self.mode = "classic".into();
        } else {
            self.mode = "enhanced".into();
            self.scale = p.scale;
            self.aspect = p.aspect.label().into();
        }
    }
}

impl Settings {
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }
}
