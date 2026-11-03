//! Application state machine: screens, menus, sequences and HUD (DESIGN §7.6).

pub mod assets;
pub mod present;
pub mod scene;
pub mod screen;
pub mod settings;
pub mod viewer;

use screen::Screen;

/// The first screen of a normal start (provisional until the title sequence lands).
pub fn start(assets: &assets::Assets) -> Box<dyn Screen> {
    Box::new(viewer::Viewer::new(assets, 0))
}
