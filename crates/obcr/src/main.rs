//! OBCR: a modern source port of Operation Body Count.
//!
//! Composition root: settings, data discovery, platform, audio and the
//! fixed-step 70 Hz loop driving the screen stack (DESIGN §6.4).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;
use obc_audio::{AudioCommand, Mixer, Volumes};
use obc_core::input::InputState;
use obc_formats::{DataDir, datadir};
use obc_platform::{AUDIO_RATE, AudioSource, Platform, VideoSettings};
use obc_render::Framebuffer;
use obc_ui::assets::Assets;
use obc_ui::present::{Aspect, LayerScaler, Presentation, blit_layer};
use obc_ui::screen::{Ctx, Frame, Requests, Screen, Transition};
use obc_ui::settings::Settings;

#[derive(Parser)]
#[command(version, about = "OBCR: a modern source port of Operation Body Count")]
struct Cli {
    /// Game data directory.
    #[arg(long)]
    data: Option<PathBuf>,
    /// Settings file (default: the OS config directory).
    #[arg(long)]
    config: Option<PathBuf>,
    /// Original 320×200 presentation.
    #[arg(long)]
    classic: bool,
    /// Enhanced resolution multiple (2..=8).
    #[arg(long)]
    scale: Option<usize>,
    /// Enhanced aspect: 4:3, 16:10, 16:9, 21:9.
    #[arg(long)]
    aspect: Option<String>,
    /// Enhanced xBRZ texture factor: 2–6, or 0 for the original texels.
    #[arg(long)]
    xbrz: Option<usize>,
    /// Enhanced xBRZ for the status bar, menus and pictures: on or off.
    #[arg(long)]
    xbrz_screens: Option<String>,
    /// Enhanced floors and ceilings: smooth or original.
    #[arg(long)]
    floors: Option<String>,
    #[arg(long)]
    fullscreen: bool,
    #[arg(long)]
    windowed: bool,
    /// Developer map viewer starting at this map slot.
    #[arg(long)]
    viewer: Option<usize>,
    /// Developer: start a new game directly on this map (0-based).
    #[arg(long)]
    play: Option<i16>,
    /// The original's cheat parameter: Alt+LShift+B in game turns on bogus mode.
    #[arg(long)]
    nsr250: bool,
    /// Game type for `--play`: 0–2 difficulty, 3 Random.
    #[arg(long, default_value_t = 1)]
    difficulty: i16,
    /// Headless: run this many tics, write the frame to `--shot-out`, exit.
    #[arg(long)]
    shot: Option<u32>,
    #[arg(long, default_value = "shot.png")]
    shot_out: PathBuf,
    /// Keys held during a headless run (PC scan codes in hex, comma separated).
    #[arg(long, value_delimiter = ',')]
    shot_keys: Vec<String>,
    /// Headless: press and release a key at a tic, as `CODE@TIC` (hex scan code).
    #[arg(long)]
    shot_press: Vec<String>,
    /// Headless: run at 70 tics per second instead of as fast as possible
    /// (network tests need real time).
    #[arg(long)]
    shot_realtime: bool,
    /// Start a network game directly: N (network), T (teams) or M (direct).
    #[arg(long)]
    net: Option<String>,
    /// UDP port (the original's IPX socket; default 16387 = 0x4003).
    #[arg(long)]
    net_port: Option<u16>,
    /// Address to call (repeatable), `host` or `host:port`.
    #[arg(long)]
    net_join: Vec<String>,
    /// Character or team: 0 Agent / Team 1, 1 Terrorist 1 / Team 2,
    /// 2 Terrorist 2 / Team 3.
    #[arg(long, default_value_t = 0)]
    net_choice: i16,
    /// Testing: drop this percentage of outgoing datagrams.
    #[arg(long, default_value_t = 0)]
    net_loss: u8,
    /// Developer: respawn at `X,Y,ANGLE` instead of a random spot.
    #[arg(long)]
    net_place: Option<String>,
    /// Open the network setup screen for `--net` instead of starting at once.
    #[arg(long)]
    net_setup: bool,
}

/// One tic: 1/70 s (`TICRATE`).
const TIC: Duration = Duration::from_nanos(1_000_000_000 / 70);
/// Catch-up cap after a stall.
const MAX_TICS_PER_FRAME: u32 = 10;

struct MixerSource(Mixer);

impl AudioSource for MixerSource {
    fn fill(&mut self, out: &mut [f32]) {
        self.0.fill(out);
    }
}

fn config_path(cli: &Cli) -> PathBuf {
    cli.config.clone().unwrap_or_else(|| {
        directories::ProjectDirs::from("", "", "OBCR")
            .map(|d| d.config_dir().join("obcr.toml"))
            .unwrap_or_else(|| PathBuf::from("obcr.toml"))
    })
}

/// Saved games: `<user data dir>/saves`, or next to an explicit `--config`.
fn save_dir(cli: &Cli) -> PathBuf {
    match &cli.config {
        Some(c) => c.parent().unwrap_or(std::path::Path::new(".")).join("saves"),
        None => directories::ProjectDirs::from("", "", "OBCR")
            .map(|d| d.data_dir().join("saves"))
            .unwrap_or_else(|| PathBuf::from("saves")),
    }
}

fn main() {
    if let Err(e) = run() {
        let msg = format!("{e:#}");
        eprintln!("OBCR error: {msg}");
        Platform::error_box("OBCR", &msg);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let cfg_path = config_path(&cli);
    let saves = save_dir(&cli);
    let mut settings = Settings::load(&cfg_path);
    settings.game.nsr250 = cli.nsr250;
    if cli.classic {
        settings.video.mode = "classic".into();
    }
    if let Some(s) = cli.scale {
        settings.video.mode = "enhanced".into();
        settings.video.scale = s;
    }
    if let Some(a) = &cli.aspect {
        settings.video.mode = "enhanced".into();
        settings.video.aspect = a.clone();
    }
    if let Some(n) = cli.xbrz {
        settings.video.xbrz = n;
    }
    if let Some(v) = &cli.xbrz_screens {
        settings.video.xbrz_screens = match v.as_str() {
            "on" => true,
            "off" => false,
            other => anyhow::bail!("--xbrz-screens wants on or off, not {other}"),
        };
    }
    if let Some(v) = &cli.floors {
        settings.video.smooth_floors = match v.as_str() {
            "smooth" => true,
            "original" => false,
            other => anyhow::bail!("--floors wants smooth or original, not {other}"),
        };
    }
    if cli.fullscreen {
        settings.video.fullscreen = true;
    }
    if cli.windowed {
        settings.video.fullscreen = false;
    }

    let configured = settings.data_dir.as_deref().map(PathBuf::from);
    let dir = datadir::discover(cli.data.as_deref(), configured.as_deref()).context(
        "Operation Body Count data not found. Use --data <dir> or set OBC_DATA_DIR to your game directory.",
    )?;
    let mut assets = Assets::load(DataDir::open(&dir)?).context("loading game data")?;
    assets.set_upscale(settings.video.texture_scale());

    if let Some(p) = cli.net_port {
        settings.net.port = p;
    }
    settings.net.loss_percent = cli.net_loss.min(100);
    let net_kind = match cli.net.as_deref().map(str::to_ascii_uppercase).as_deref() {
        None => None,
        Some("N") => Some(4),
        Some("T") => Some(5),
        Some("M") => Some(6),
        Some(other) => anyhow::bail!("--net wants N, T or M, not {other}"),
    };
    let first: Box<dyn Screen> = match (cli.viewer, cli.play) {
        _ if net_kind.is_some() && cli.net_setup => {
            Box::new(obc_ui::netui::SetupBoot { kind: net_kind.unwrap_or(4) })
        }
        _ if net_kind.is_some() => Box::new(obc_ui::netui::NetBoot {
            kind: net_kind.unwrap_or(4),
            choice: cli.net_choice.clamp(0, 2),
            join: cli.net_join.clone(),
            place: match &cli.net_place {
                None => None,
                Some(t) => {
                    let v: Vec<i32> = t.split(',').filter_map(|s| s.trim().parse().ok()).collect();
                    let [x, y, a] = v[..] else { anyhow::bail!("--net-place wants X,Y,ANGLE") };
                    Some((x, y, a as i16))
                }
            },
        }),
        (Some(slot), _) => Box::new(obc_ui::viewer::Viewer::new(&assets, slot)),
        (None, Some(mapon)) => Box::new(obc_ui::game::Boot { difficulty: cli.difficulty, mapon }),
        (None, None) => obc_ui::start(&assets),
    };
    let mut stack: Vec<Box<dyn Screen>> = vec![first];

    if let Some(tics) = cli.shot {
        return headless_shot(&assets, &mut settings, &mut stack, tics, &cli, &saves);
    }

    let mut platform = Platform::new(VideoSettings {
        title: "OBCR — Operation Body Count".into(),
        width: settings.video.window_width,
        height: settings.video.window_height,
        fullscreen: settings.video.fullscreen,
        vsync: settings.video.vsync,
        smooth: settings.video.smooth,
        integer_scale: settings.video.integer_scale,
    })
    .map_err(|e| anyhow::anyhow!("{e}"))?;

    let (mixer, audio) = Mixer::new(AUDIO_RATE);
    if let Err(e) = platform.start_audio(Box::new(MixerSource(mixer))) {
        eprintln!("audio unavailable: {e}");
    }
    let a = &settings.audio;
    audio.send(AudioCommand::SetVolumes(Volumes { master: a.master, sfx: a.sfx, music: a.music }));

    let mut input = InputState::default();
    let mut frame = Framebuffer::new(320, 200);
    let mut layer = Framebuffer::new(320, 200);
    let mut scaler = LayerScaler::default();
    let mut rgba = Vec::new();
    let mut last = Instant::now();
    let mut acc = Duration::ZERO;

    'main: loop {
        assets.set_upscale(settings.video.texture_scale());
        if !platform.pump(&mut input) {
            break;
        }
        let now = Instant::now();
        acc += now - last;
        last = now;
        let mut tics = 0;
        while acc >= TIC {
            acc -= TIC;
            tics += 1;
            if tics > MAX_TICS_PER_FRAME {
                acc = Duration::ZERO;
                break;
            }
            let mut requests = Requests::default();
            let alive = {
                let mut ctx = Ctx {
                    assets: &assets,
                    audio: &audio,
                    input: &input,
                    settings: &mut settings,
                    requests: &mut requests,
                    save_dir: &saves,
                };
                tick(&mut stack, &mut ctx)
            };
            input.end_frame();
            apply_requests(&mut platform, &requests, &settings);
            if !alive {
                break 'main;
            }
        }

        let p = presentation(&settings);
        let alpha = (acc.as_secs_f32() / TIC.as_secs_f32()).clamp(0.0, 1.0);
        let mut target = Target { frame: &mut frame, layer: &mut layer, scaler: &mut scaler };
        let palette = compose(&mut stack, &assets, &settings, &mut target, alpha);
        let (fw, fh) = (frame.width, frame.height);
        frame.to_rgba(&palette, &mut rgba);
        platform
            .present(&rgba, fw as u32, fh as u32, p.aspect.ratio())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if tics == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    if let Err(e) = settings.save(&cfg_path) {
        eprintln!("could not save settings to {}: {e}", cfg_path.display());
    }
    Ok(())
}

/// Runs one tic of the top screen and applies its transition. Returns false to quit.
fn tick(stack: &mut Vec<Box<dyn Screen>>, ctx: &mut Ctx) -> bool {
    let Some(top) = stack.last_mut() else { return false };
    match top.tick(ctx) {
        Transition::None => {}
        Transition::Push(s) => stack.push(s),
        Transition::Pop => {
            stack.pop();
        }
        Transition::Replace(s) => {
            stack.pop();
            stack.push(s);
        }
        Transition::Reset(s) => {
            stack.clear();
            stack.push(s);
        }
        Transition::Quit => return false,
    }
    !stack.is_empty()
}

/// What a frame is composed into.
struct Target<'a> {
    frame: &'a mut Framebuffer,
    layer: &'a mut Framebuffer,
    scaler: &'a mut LayerScaler,
}

/// Draws the visible screens and composites the 2D layer. Returns the palette.
fn compose(
    stack: &mut [Box<dyn Screen>],
    assets: &Assets,
    settings: &Settings,
    t: &mut Target,
    alpha: f32,
) -> obc_render::palette::Palette {
    let p = presentation(settings);
    let (fw, fh) = p.frame_size();
    t.frame.resize(fw, fh);
    t.frame.begin_frame();
    let smooth_floors = settings.video.smooth_floors();
    let xbrz_screens = settings.video.screens_xbrz();
    let mut palette = assets.gamepal;
    let mut scroll = 0;
    let first_visible = stack.iter().rposition(|s| s.opaque()).unwrap_or(0);
    for screen in &mut stack[first_visible..] {
        let mut out = Frame {
            presentation: p,
            frame: t.frame,
            layer: t.layer,
            layer_key: None,
            palette,
            scroll: 0,
            alpha,
            smooth_floors,
        };
        screen.draw(assets, &mut out);
        palette = out.palette;
        scroll = out.scroll;
        let key = out.layer_key;
        if xbrz_screens {
            t.scaler.blit(t.frame, t.layer, p, key, &palette);
        } else {
            blit_layer(t.frame, t.layer, p, key);
        }
    }
    // Scroll up; the lines past the page show black.
    t.frame.scroll_up(scroll * p.scale);
    palette
}

fn headless_shot(
    assets: &Assets,
    settings: &mut Settings,
    stack: &mut Vec<Box<dyn Screen>>,
    tics: u32,
    cli: &Cli,
    saves: &std::path::Path,
) -> Result<()> {
    let (_mixer, audio) = Mixer::new(AUDIO_RATE);
    let mut input = InputState::default();
    for k in &cli.shot_keys {
        let code =
            u8::from_str_radix(k.trim_start_matches("0x"), 16).context("bad --shot-keys code")?;
        input.key(code, true, false);
    }
    let mut presses = Vec::new();
    for p in &cli.shot_press {
        let (code, tic) = p.split_once('@').context("--shot-press wants CODE@TIC")?;
        let code = u8::from_str_radix(code.trim_start_matches("0x"), 16)
            .context("bad --shot-press code")?;
        presses.push((code, tic.parse::<u32>().context("bad --shot-press tic")?));
    }
    let start = Instant::now();
    for t in 0..tics {
        if cli.shot_realtime {
            let due = start + TIC * t;
            if let Some(wait) = due.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
        }
        for &(code, at) in &presses {
            if at == t {
                input.key(code, true, false);
            } else if at + 1 == t {
                input.key(code, false, false);
            }
        }
        let mut requests = Requests::default();
        let mut ctx = Ctx {
            assets,
            audio: &audio,
            input: &input,
            settings,
            requests: &mut requests,
            save_dir: saves,
        };
        if !tick(stack, &mut ctx) {
            break;
        }
        input.events.clear();
    }
    let mut frame = Framebuffer::new(320, 200);
    let mut layer = Framebuffer::new(320, 200);
    let mut scaler = LayerScaler::default();
    let mut target = Target { frame: &mut frame, layer: &mut layer, scaler: &mut scaler };
    let palette = compose(stack, assets, settings, &mut target, 0.0);
    let mut rgba = Vec::new();
    frame.to_rgba(&palette, &mut rgba);
    let file = std::io::BufWriter::new(std::fs::File::create(&cli.shot_out)?);
    let mut enc = png::Encoder::new(file, frame.width as u32, frame.height as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgba)?;
    println!("wrote {} ({}x{})", cli.shot_out.display(), frame.width, frame.height);
    if let Some(line) = stack
        .iter_mut()
        .rev()
        .find_map(|s| s.game_mut())
        .and_then(|g| obc_ui::netui::summary(g.world()))
    {
        println!("{line}");
    }
    Ok(())
}

fn presentation(settings: &Settings) -> Presentation {
    let p = settings.video.presentation();
    if p.scale <= 1 && p.aspect == Aspect::Standard { Presentation::CLASSIC } else { p }
}

fn apply_requests(platform: &mut Platform, r: &Requests, settings: &Settings) {
    if let Some(on) = r.relative_mouse {
        platform.set_relative_mouse(on);
    }
    if let Some(on) = r.text_input {
        platform.set_text_input(on);
    }
    if let Some(on) = r.fullscreen {
        platform.set_fullscreen(on);
    }
    if let Some((lo, hi, ms)) = r.rumble
        && settings.input.rumble
    {
        platform.rumble(lo, hi, ms);
    }
    if r.apply_video {
        platform.set_smooth(settings.video.smooth);
        platform.set_integer_scale(settings.video.integer_scale);
    }
}
