//! SDL3 platform layer: window, presentation, input, audio device (DESIGN §6.3).
//!
//! The game renders RGBA frames of any size; [`Platform::present`] scales them
//! into the window at their intended display aspect ratio. Input arrives as an
//! [`InputState`] keyed by original PC scan codes, plus mouse and gamepad state.

mod input;
mod scancodes;

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream, AudioStreamWithCallback};
use sdl3::event::{Event, WindowEvent};
use sdl3::gamepad::Gamepad;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::rect::Rect;
use sdl3::render::{ScaleMode, Texture, TextureCreator, WindowCanvas};
use sdl3::video::WindowContext;

pub use input::{GamepadState, InputState, KeyEvent, pad};
pub use scancodes::sc;

/// Error from the platform layer.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PlatformError(String);

fn err(e: impl std::fmt::Display) -> PlatformError {
    PlatformError(e.to_string())
}

/// Window and presentation settings.
#[derive(Debug, Clone)]
pub struct VideoSettings {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    /// Linear filtering when scaling (otherwise nearest).
    pub smooth: bool,
    /// Scale only by whole multiples (sharper, smaller image).
    pub integer_scale: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            title: "OBCR".into(),
            width: 1280,
            height: 960,
            fullscreen: false,
            vsync: true,
            smooth: false,
            integer_scale: false,
        }
    }
}

/// A source of interleaved stereo `f32` samples, run on the audio thread.
pub trait AudioSource: Send + 'static {
    fn fill(&mut self, out: &mut [f32]);
}

struct AudioBridge {
    source: Box<dyn AudioSource>,
    buf: Vec<f32>,
}

impl AudioCallback<f32> for AudioBridge {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        let n = requested.max(0) as usize;
        self.buf.resize(n, 0.0);
        self.source.fill(&mut self.buf);
        let _ = stream.put_data_f32(&self.buf);
    }
}

/// Output sample rate requested from the device.
pub const AUDIO_RATE: u32 = 48_000;

/// The SDL context, window, renderer and devices.
pub struct Platform {
    _sdl: sdl3::Sdl,
    video: sdl3::VideoSubsystem,
    canvas: WindowCanvas,
    creator: &'static TextureCreator<WindowContext>,
    texture: Option<(Texture<'static>, u32, u32)>,
    events: sdl3::EventPump,
    gamepad_sys: sdl3::GamepadSubsystem,
    gamepads: Vec<Gamepad>,
    mouse: sdl3::mouse::MouseUtil,
    audio_sys: sdl3::AudioSubsystem,
    audio: Option<AudioStreamWithCallback<AudioBridge>>,
    settings: VideoSettings,
    relative_mouse: bool,
}

impl Platform {
    pub fn new(settings: VideoSettings) -> Result<Self, PlatformError> {
        sdl3::hint::set("SDL_RENDER_VSYNC", if settings.vsync { "1" } else { "0" });
        let sdl = sdl3::init().map_err(err)?;
        let video = sdl.video().map_err(err)?;
        let mut builder = video.window(&settings.title, settings.width, settings.height);
        builder.resizable().position_centered().high_pixel_density();
        if settings.fullscreen {
            builder.fullscreen();
        }
        let window = builder.build().map_err(err)?;
        let canvas = window.into_canvas();
        // The texture creator lives as long as the process; leaking one avoids a
        // self-referential struct for `Texture<'_>`.
        let creator: &'static TextureCreator<WindowContext> =
            Box::leak(Box::new(canvas.texture_creator()));
        let events = sdl.event_pump().map_err(err)?;
        let gamepad_sys = sdl.gamepad().map_err(err)?;
        let mouse = sdl.mouse();
        let audio_sys = sdl.audio().map_err(err)?;
        let mut platform = Self {
            _sdl: sdl,
            video,
            canvas,
            creator,
            texture: None,
            events,
            gamepad_sys,
            gamepads: Vec::new(),
            mouse,
            audio_sys,
            audio: None,
            settings,
            relative_mouse: false,
        };
        platform.open_gamepads();
        Ok(platform)
    }

    fn open_gamepads(&mut self) {
        if let Ok(ids) = self.gamepad_sys.gamepads() {
            for id in ids {
                if !self.gamepads.iter().any(|g| g.id() == Ok(id))
                    && let Ok(g) = self.gamepad_sys.open(id)
                {
                    self.gamepads.push(g);
                }
            }
        }
    }

    /// Starts audio output. Samples are interleaved stereo at [`AUDIO_RATE`].
    pub fn start_audio(&mut self, source: Box<dyn AudioSource>) -> Result<(), PlatformError> {
        let spec = AudioSpec {
            freq: Some(AUDIO_RATE as i32),
            channels: Some(2),
            format: Some(AudioFormat::f32_sys()),
        };
        let stream = self
            .audio_sys
            .open_playback_stream(&spec, AudioBridge { source, buf: Vec::new() })
            .map_err(err)?;
        stream.resume().map_err(err)?;
        self.audio = Some(stream);
        Ok(())
    }

    /// Captures the mouse for turning (relative mode) or releases it for menus.
    pub fn set_relative_mouse(&mut self, on: bool) {
        if on != self.relative_mouse {
            self.mouse.set_relative_mouse_mode(self.canvas.window(), on);
            self.relative_mouse = on;
        }
    }

    pub fn set_fullscreen(&mut self, on: bool) {
        self.settings.fullscreen = on;
        let _ = self.canvas.window_mut().set_fullscreen(on);
    }

    pub fn is_fullscreen(&self) -> bool {
        self.settings.fullscreen
    }

    pub fn set_smooth(&mut self, smooth: bool) {
        self.settings.smooth = smooth;
        if let Some((t, _, _)) = &mut self.texture {
            t.set_scale_mode(if smooth { ScaleMode::Linear } else { ScaleMode::Nearest });
        }
    }

    pub fn set_integer_scale(&mut self, on: bool) {
        self.settings.integer_scale = on;
    }

    /// Window size in physical pixels.
    pub fn output_size(&self) -> (u32, u32) {
        self.canvas.output_size().unwrap_or((self.settings.width, self.settings.height))
    }

    /// Pumps SDL events into `input`. Returns false when the user closed the window.
    pub fn pump(&mut self, input: &mut InputState) -> bool {
        let mut alive = true;
        let mut gamepads_changed = false;
        let mut toggle_fullscreen = false;
        for event in self.events.poll_iter() {
            match event {
                Event::Quit { .. } => alive = false,
                Event::Window { win_event: WindowEvent::FocusLost, .. } => input.release_all(),
                Event::KeyDown { scancode: Some(s), repeat, keymod, .. } => {
                    let alt = keymod
                        .intersects(sdl3::keyboard::Mod::LALTMOD | sdl3::keyboard::Mod::RALTMOD);
                    if s == sdl3::keyboard::Scancode::Return && alt && !repeat {
                        toggle_fullscreen = true;
                        continue;
                    }
                    if let Some(code) = scancodes::to_pc(s) {
                        input.key(code, true, repeat);
                    }
                }
                Event::KeyUp { scancode: Some(s), .. } => {
                    if let Some(code) = scancodes::to_pc(s) {
                        input.key(code, false, false);
                    }
                }
                Event::TextInput { text, .. } => input.text.push_str(&text),
                Event::MouseMotion { xrel, yrel, .. } => {
                    input.mouse_dx += xrel;
                    input.mouse_dy += yrel;
                }
                Event::MouseButtonDown { mouse_btn, .. } => {
                    input.mouse_button(mouse_btn as u8, true)
                }
                Event::MouseButtonUp { mouse_btn, .. } => {
                    input.mouse_button(mouse_btn as u8, false)
                }
                Event::GamepadAdded { .. } | Event::GamepadRemoved { .. } => {
                    gamepads_changed = true
                }
                _ => {}
            }
        }
        if toggle_fullscreen {
            self.set_fullscreen(!self.settings.fullscreen);
        }
        if gamepads_changed {
            self.gamepads.retain(|g| g.connected());
            self.open_gamepads();
        }
        input.pad = self.read_gamepads();
        alive
    }

    fn read_gamepads(&self) -> GamepadState {
        use sdl3::gamepad::{Axis, Button};
        let mut st = GamepadState { connected: !self.gamepads.is_empty(), ..Default::default() };
        let norm = |v: i16| (v as f32 / 32767.0).clamp(-1.0, 1.0);
        for g in &self.gamepads {
            st.left_x += norm(g.axis(Axis::LeftX));
            st.left_y += norm(g.axis(Axis::LeftY));
            st.right_x += norm(g.axis(Axis::RightX));
            st.right_y += norm(g.axis(Axis::RightY));
            st.left_trigger = st.left_trigger.max(norm(g.axis(Axis::TriggerLeft)));
            st.right_trigger = st.right_trigger.max(norm(g.axis(Axis::TriggerRight)));
            for (button, bit) in [
                (Button::South, pad::SOUTH),
                (Button::East, pad::EAST),
                (Button::West, pad::WEST),
                (Button::North, pad::NORTH),
                (Button::Back, pad::BACK),
                (Button::Start, pad::START),
                (Button::LeftStick, pad::LEFT_STICK),
                (Button::RightStick, pad::RIGHT_STICK),
                (Button::LeftShoulder, pad::LEFT_SHOULDER),
                (Button::RightShoulder, pad::RIGHT_SHOULDER),
                (Button::DPadUp, pad::DPAD_UP),
                (Button::DPadDown, pad::DPAD_DOWN),
                (Button::DPadLeft, pad::DPAD_LEFT),
                (Button::DPadRight, pad::DPAD_RIGHT),
            ] {
                if g.button(button) {
                    st.buttons |= bit;
                }
            }
        }
        st
    }

    /// Displays an RGBA frame (`width × height`), fitted into the window at
    /// `display_aspect` (width / height of the intended picture, e.g. 4/3 for
    /// the original 320×200 screen).
    pub fn present(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        display_aspect: f32,
    ) -> Result<(), PlatformError> {
        if self.texture.as_ref().is_none_or(|(_, w, h)| (*w, *h) != (width, height)) {
            let mut t = self
                .creator
                .create_texture_streaming(PixelFormat::RGBA32, width, height)
                .map_err(err)?;
            t.set_scale_mode(if self.settings.smooth {
                ScaleMode::Linear
            } else {
                ScaleMode::Nearest
            });
            self.texture = Some((t, width, height));
        }
        let Some((texture, _, _)) = &mut self.texture else { unreachable!() };
        texture.update(None, rgba, width as usize * 4).map_err(err)?;
        let (ow, oh) = self.canvas.output_size().map_err(err)?;
        let dst = fit_rect(ow, oh, width, height, display_aspect, self.settings.integer_scale);
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();
        self.canvas.copy(texture, None, Some(dst.into())).map_err(err)?;
        self.canvas.present();
        Ok(())
    }

    /// Shows a blocking error dialog (used for fatal errors).
    pub fn error_box(title: &str, message: &str) {
        let _ = sdl3::messagebox::show_simple_message_box(
            sdl3::messagebox::MessageBoxFlag::ERROR,
            title,
            message,
            None::<&sdl3::video::Window>,
        );
    }

    pub fn video(&self) -> &sdl3::VideoSubsystem {
        &self.video
    }

    /// Enables or disables SDL text-input events (for name entry).
    pub fn set_text_input(&mut self, on: bool) {
        let text = self.video.text_input();
        if on {
            text.start(self.canvas.window());
        } else {
            text.stop(self.canvas.window());
        }
    }

    /// Rumbles every connected gamepad (`low`/`high` 0..=1) for `ms` milliseconds.
    pub fn rumble(&mut self, low: f32, high: f32, ms: u32) {
        let to16 = |v: f32| (v.clamp(0.0, 1.0) * 65535.0) as u16;
        for g in &mut self.gamepads {
            let _ = g.set_rumble(to16(low), to16(high), ms);
        }
    }
}

/// Largest rectangle with the picture's display aspect that fits the window, centered.
fn fit_rect(ow: u32, oh: u32, w: u32, h: u32, aspect: f32, integer: bool) -> Rect {
    let (ow_f, oh_f) = (ow as f32, oh as f32);
    let (mut dw, mut dh) =
        if ow_f / oh_f > aspect { (oh_f * aspect, oh_f) } else { (ow_f, ow_f / aspect) };
    if integer {
        // Whole multiples of the source height (keeps scanlines uniform).
        let k = (dh / h as f32).floor().max(1.0);
        dh = h as f32 * k;
        dw = dh * aspect;
        let _ = w;
    }
    let x = ((ow_f - dw) / 2.0).round() as i32;
    let y = ((oh_f - dh) / 2.0).round() as i32;
    Rect::new(x, y, dw.round().max(1.0) as u32, dh.round().max(1.0) as u32)
}

#[cfg(test)]
mod tests {
    use super::fit_rect;

    #[test]
    fn letterboxes_four_by_three_in_widescreen() {
        let r = fit_rect(1920, 1080, 320, 200, 4.0 / 3.0, false);
        assert_eq!((r.x(), r.y(), r.width(), r.height()), (240, 0, 1440, 1080));
    }

    #[test]
    fn integer_scale_uses_whole_multiples() {
        let r = fit_rect(1920, 1080, 320, 200, 4.0 / 3.0, true);
        assert_eq!(r.height(), 1000);
    }
}
