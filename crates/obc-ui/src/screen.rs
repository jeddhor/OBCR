//! The non-blocking screen stack that replaces the original's nested loops
//! (DESIGN §7.6).

use obc_audio::MixerHandle;
use obc_core::input::InputState;
use obc_render::Framebuffer;
use obc_render::palette::Palette;

use crate::assets::Assets;
use crate::present::Presentation;
use crate::settings::Settings;

/// What a screen wants after a tic.
pub enum Transition {
    None,
    Push(Box<dyn Screen>),
    Pop,
    Replace(Box<dyn Screen>),
    /// Clears the whole stack, then pushes this screen.
    Reset(Box<dyn Screen>),
    Quit,
}

/// Per-tic context.
pub struct Ctx<'a> {
    pub assets: &'a Assets,
    pub audio: &'a MixerHandle,
    pub input: &'a InputState,
    pub settings: &'a mut Settings,
    /// Requests to the platform layer, applied after the tic.
    pub requests: &'a mut Requests,
    /// Where saved games go (the OS user data directory).
    pub save_dir: &'a std::path::Path,
}

/// Platform changes requested by screens.
#[derive(Debug, Default)]
pub struct Requests {
    pub relative_mouse: Option<bool>,
    pub text_input: Option<bool>,
    pub fullscreen: Option<bool>,
    pub rumble: Option<(f32, f32, u32)>,
    pub apply_video: bool,
}

/// What a screen draws into.
pub struct Frame<'a> {
    pub presentation: Presentation,
    /// Full frame (Classic: 320×200; Enhanced: high resolution).
    pub frame: &'a mut Framebuffer,
    /// 320×200 2D layer, upscaled over the frame after drawing.
    pub layer: &'a mut Framebuffer,
    /// Layer color treated as transparent when compositing, if any.
    pub layer_key: Option<u8>,
    /// The layer is the in-game status bar and overlays (XBRZ HUD), not a
    /// menu, picture or sequence (XBRZ MENUS).
    pub layer_hud: bool,
    pub palette: Palette,
    /// Lines (original pixels) the finished frame scrolls up by: the tremor
    /// shake moves the CRTC start address, status bar included.
    pub scroll: usize,
    /// How far into the current 70 Hz tic this draw happens (0..1), for
    /// Enhanced interpolation.
    pub alpha: f32,
    /// Enhanced SMOOTH FLOORS for the 3D view (ADR-019).
    pub smooth_floors: bool,
}

pub trait Screen {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition;
    fn draw(&mut self, assets: &Assets, out: &mut Frame);
    /// Screens below an opaque screen are not drawn.
    fn opaque(&self) -> bool {
        true
    }
    /// The running game, if this screen is one (tools and tests).
    fn game_mut(&mut self) -> Option<&mut crate::game::Game> {
        None
    }
    /// A short name for logs and tests.
    fn name(&self) -> &'static str {
        "screen"
    }
}
