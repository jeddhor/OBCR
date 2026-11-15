//! Data-gated: a format-2 mod built here (never stored) with its own wall,
//! sprite, sound, music and arenas (docs/MOD_ASSETS_AND_ARENAS.md §3.7).

use std::path::Path;

use obc_audio::Mixer;
use obc_core::input::InputState;
use obc_formats::DataDir;
use obc_formats::maps::{Map, write_archive};
use obc_render::Framebuffer;
use obc_render::view3d::Textures;
use obc_ui::assets::Assets;
use obc_ui::game::Game;
use obc_ui::modmedia::{Layered, ModMedia};
use obc_ui::present::Presentation;
use obc_ui::screen::{Ctx, Frame, Requests, Screen};
use obc_ui::settings::Settings;

/// A walled room, the player at (2, 2) facing east at the stairs (code 114,
/// wall page 113) in the next cell; `enemies` soldiers on row 10.
fn room(name: &str, enemies: usize) -> Map {
    let n = 64 * 64;
    let mut p0 = vec![144u16; n];
    for i in 0..64 {
        for j in [i, 63 * 64 + i, i * 64, i * 64 + 63] {
            p0[j] = 1;
        }
    }
    p0[2 * 64 + 3] = 114;
    let mut p1 = vec![18u16; n];
    p1[2 * 64 + 2] = 20;
    for e in 0..enemies {
        p1[10 * 64 + 10 + e] = 116;
    }
    Map { name: name.into(), width: 64, height: 64, planes: [p0, p1, vec![0; n]] }
}

fn png(rgba: &[[u8; 4]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, 64, 64);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut w = e.write_header().unwrap();
    w.write_image_data(&rgba.concat()).unwrap();
    w.finish().unwrap();
    out
}

fn wav() -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 22_050,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut c = std::io::Cursor::new(Vec::new());
    let mut w = hound::WavWriter::new(&mut c, spec).unwrap();
    for i in 0..2205 {
        w.write_sample(if i % 40 < 20 { 9000i16 } else { -9000 }).unwrap();
    }
    w.finalize().unwrap();
    c.into_inner()
}

const MAGENTA: [u8; 4] = [255, 0, 255, 255];

fn write_mod(dir: &Path, arena_enemies: usize) {
    for sub in ["walls", "sprites", "sounds", "music"] {
        std::fs::create_dir_all(dir.join(sub)).unwrap();
    }
    let (a, b, c) = (room("ROOM", 0), room("PIT", arena_enemies), room("YARD", 0));
    let (head, body) = write_archive(&[Some(&a), Some(&b), Some(&c)], 0xABCD);
    std::fs::write(dir.join("MAPHEAD.BC"), head).unwrap();
    std::fs::write(dir.join("MAPTEMP.BC"), body).unwrap();
    std::fs::write(dir.join("walls/113.png"), png(&[MAGENTA; 64 * 64])).unwrap();
    // A sprite: an opaque green square in a transparent field.
    let mut img = vec![[0u8, 0, 0, 0]; 64 * 64];
    for y in 20..40 {
        for x in 20..40 {
            img[y * 64 + x] = [0, 255, 0, 255];
        }
    }
    std::fs::write(dir.join("sprites/450.png"), png(&img)).unwrap();
    std::fs::write(dir.join("sounds/05.wav"), wav()).unwrap();
    std::fs::write(
        dir.join("music/tone.ogg"),
        include_bytes!("../../obc-formats/tests/data/tone.ogg"),
    )
    .unwrap();
    std::fs::write(
        dir.join("mod.toml"),
        "format = 2\n[mod]\nid = \"art-test\"\ntitle = \"Art Test\"\nversion = \"1\"\n\
         [[level]]\nmap = 0\nmusic = \"tone.ogg\"\nsequence = \"none\"\n\
         [[arena]]\nmap = 1\nname = \"Pit\"\n[[arena]]\nmap = 2\n",
    )
    .unwrap();
}

struct H {
    assets: Assets,
    settings: Settings,
    saves: std::path::PathBuf,
    audio: obc_audio::MixerHandle,
    _mixer: Mixer,
}

impl H {
    fn with_ctx<R>(&mut self, f: impl FnOnce(&mut Ctx) -> R) -> R {
        let input = InputState::default();
        let mut requests = Requests::default();
        let mut ctx = Ctx {
            assets: &self.assets,
            audio: &self.audio,
            input: &input,
            settings: &mut self.settings,
            requests: &mut requests,
            save_dir: &self.saves,
        };
        f(&mut ctx)
    }
}

/// The 3D view's centre pixel after drawing `g` in Classic.
fn centre_pixel(g: &mut Game, a: &Assets) -> u8 {
    let mut frame = Framebuffer::new(320, 200);
    let mut layer = Framebuffer::new(320, 200);
    let mut out = Frame {
        presentation: Presentation::CLASSIC,
        frame: &mut frame,
        layer: &mut layer,
        layer_key: None,
        layer_hud: false,
        palette: a.gamepal,
        scroll: 0,
        alpha: 0.0,
        smooth_floors: false,
    };
    g.draw(a, &mut out);
    frame.pixels[(80 - 4) * 320 + 160]
}

#[test]
fn a_mod_brings_its_own_art_sounds_music_and_arenas() {
    let Some(dir) = obc_testdata::data_dir() else {
        eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
        return;
    };
    let root = std::env::temp_dir().join(format!("obcr-modart-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let saves = root.join("data").join("saves");
    let mod_dir = root.join("data").join("mods").join("art-test");
    write_mod(&mod_dir, 0);

    let assets = Assets::load(DataDir::open(dir).unwrap()).unwrap();
    let (mixer, audio) = Mixer::new(48_000);
    let mut h = H { assets, settings: Settings::default(), saves, audio, _mixer: mixer };

    let (m, problems) = obc_ui::campaign::check(&mod_dir).unwrap();
    assert!(problems.iter().all(|p| !p.error), "{problems:?}");
    assert_eq!((m.manifest.arenas.len(), m.package.assets.walls.len()), (2, 1));

    // The converted assets.
    let media = ModMedia::load(m.package.clone(), &h.assets.gamepal);
    let layered = Layered { base: &h.assets, media: &media };
    let wall = layered.wall(113).unwrap();
    let magenta = wall[0];
    assert!(wall.iter().all(|&p| p == magenta));
    assert_ne!(h.assets.wall(113).unwrap(), wall, "the game's page is untouched");
    // The nearest entry 0-254 to magenta, by the rule (the palette may have
    // no purple at all).
    let d =
        |c: [u8; 3]| (0..3).map(|k| (i32::from(c[k]) - i32::from(MAGENTA[k])).pow(2)).sum::<i32>();
    let best = (0..255).min_by_key(|&i| (d(h.assets.gamepal[i]), i)).unwrap();
    assert_eq!(usize::from(magenta), best);
    let sprite = layered.sprite(450).unwrap();
    assert_eq!((sprite.left, sprite.right), (20, 39));
    assert!(sprite.opaque[25 * 64 + 25] && !sprite.opaque[5 * 64 + 5]);
    assert_eq!(layered.sprite(451), h.assets.sprite(451), "others are the game's");
    assert_eq!(media.sounds.len(), 1);
    assert_eq!((media.sounds[0].0, media.sounds[0].2), (5, 22_050));
    assert!(media.music("TONE.ogg").is_some());

    // The mod's room: the wall in front of the player is the mod's magenta.
    h.settings.game.mod_path = Some(mod_dir.clone());
    let mut g = h.with_ctx(|ctx| Game::new(ctx, 1));
    assert_eq!(g.world().level_music_file(), Some("tone.ogg"));
    assert_eq!(centre_pixel(&mut g, &h.assets), magenta, "the mod's wall is drawn");

    // Network arenas: the mod's, in order, wrapping after the last.
    let (id_with, title) = h.with_ctx(|ctx| obc_ui::campaign::net_identity(ctx));
    assert_eq!(title.as_deref(), Some("Art Test"));
    assert!(id_with.contains("+mod:art-test:1:"), "{id_with}");
    let transport = obc_net::Transport::open(
        obc_net::Config { port: 0, broadcast: false, id: Some(9), ..Default::default() },
        std::time::Instant::now(),
    )
    .unwrap();
    let link = obc_ui::netui::NetLink::new(transport);
    let mut net = h.with_ctx(|ctx| Game::new_net(ctx, 4, obc_sim::net::NetGame::new(9, 0), link));
    let w = net.world();
    assert_eq!(w.gamestate.mapon, 50);
    assert_eq!(w.plane0[2 * 64 + 3], 114, "the mod's arena, not the original's");
    let region = w.net.as_ref().unwrap().spawn_region.clone().unwrap();
    assert!(region[5 * 64 + 5] && !region[0], "floor in, walls out");
    // Past the last arena: back to the first.
    net.world_mut().gamestate.mapon = 52;
    h.with_ctx(|ctx| net.start_level(ctx));
    assert_eq!(net.world().gamestate.mapon, 50);

    // Without the mod, the original arenas and identity.
    h.settings.game.mod_path = None;
    let (id_plain, title) = h.with_ctx(|ctx| obc_ui::campaign::net_identity(ctx));
    assert_eq!((id_plain.as_str(), title), (h.assets.release.release.id, None));

    // An arena with enemies is refused.
    write_mod(&mod_dir, 1);
    let (_, problems) = obc_ui::campaign::check(&mod_dir).unwrap();
    assert!(problems.iter().any(|p| p.error && p.message.starts_with("arena 1:")), "{problems:?}");
    let _ = std::fs::remove_dir_all(&root);
}
