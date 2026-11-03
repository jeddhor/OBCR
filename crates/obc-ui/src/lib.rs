//! Application state machine: screens, menus, sequences and HUD (DESIGN §7.6).

pub mod assets;
pub mod controls;
pub mod game;
pub mod hud;
pub mod menu;
pub mod present;
pub mod saves;
pub mod scene;
pub mod screen;
pub mod sequence;
pub mod settings;
pub mod sound;
pub mod viewer;

use screen::Screen;

/// The first screen of a normal start: intro and title, then the menu.
pub fn start(_assets: &assets::Assets) -> Box<dyn Screen> {
    Box::new(menu::Title::new())
}
