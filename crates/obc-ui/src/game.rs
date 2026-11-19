//! The in-game screen (`GameLoop` 1823:0ea3, `PlayLoop` 1942:1315).
//!
//! Owns the [`World`] and runs one original frame of two tics every other
//! 70 Hz tic (ADR-013). Simulation events drive sound, the status bar,
//! palette flashes, the tremor shake and gamepad rumble. Level ends follow
//! GAMEFLOW §5.3–5.5: exit sequence and next map, death sequence and the
//! menu, or victory.

use std::sync::Arc;

use obc_audio::AudioCommand;
use obc_render::Framebuffer;
use obc_render::palette::{Palette, expand6, faded};
use obc_render::view3d::{Scene, SplatView, View3D, build_shade_table};
use obc_sim::events::SimEvent;
use obc_sim::level::LevelSequence;
use obc_sim::world::{TicCmd, World, ex};

use crate::assets::Assets;
use crate::controls::Controls;
use crate::hud::Hud;
use crate::scene::{SceneBuffers, floor_params, weapon_view};
use crate::screen::{Ctx, Frame, Screen, Transition};
use crate::sequence::Sequence;
use crate::sound::{Listener, SoundManager};
use crate::viewer::{Layout, layout, tile_background};

/// Tics per original frame (ADR-013).
const FRAME_TICS: i16 = 2;
/// Red and white palette shifts (RENDERER.md §8.1).
const NUMREDSHIFTS: i32 = 4;
const REDSTEPS: i32 = 8;
const NUMWHITESHIFTS: i32 = 3;
const WHITESTEPS: i32 = 10;
const WHITETICS: i32 = 6;
/// `T2[c]` of the page-flip shake (DS:0x0D51): two lines up on odd counts.
const SHAKE_LINES: [usize; 2] = [0, 2];
/// The low-health blackout: 50 fade steps, one per tic.
const BLACKOUT: u32 = 50;
/// Level fade-in and exit fade-out, in tics.
const FADE_IN: u32 = 30;
const FADE_OUT: u32 = 30;
/// `Died`: scream wait cap and the two fades (dark red, then black).
const SCREAM_WAIT: u32 = 49;
const DEATH_RED: u32 = 150;
const DEATH_BLACK: u32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum After {
    NextLevel,
    /// Network warp: the same `mapon`, reloaded.
    Restart,
    /// Back to DOS (the KILLS table first in network games).
    Quit,
    /// Death or victory: the high-score check, then the menu.
    Scores,
    /// After the victory picture: F7, then the scores.
    Victory,
    Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    FadeIn(u32),
    Playing,
    /// Turning to face the killer, then waiting for the scream.
    Dying {
        target: Option<i16>,
        wait: u32,
    },
    DeathFade(u32),
    ExitFade(u32),
    /// A sequence was pushed; act when it pops.
    Sequence(After),
    /// Victory: hold the view for up to 1000 tics or a key.
    Victory(u32),
    /// Network games: the KILLS table until a key (358f:13c5).
    Kills(After),
}

pub struct Game {
    world: World,
    sound: SoundManager,
    hud: Hud,
    controls: Controls,
    view: Option<(Layout, View3D)>,
    buffers: SceneBuffers,
    splats: Vec<SplatView>,
    shade: [[u8; 256]; 2],
    phase: Phase,
    odd_tic: bool,
    damagecount: i32,
    bonuscount: i32,
    /// Palette mode: 1 normal, 2 dark (generators off).
    lighting: u8,
    shake_lines: usize,
    floor_mode: u8,
    show_health: bool,
    crosshair: bool,
    paused: bool,
    /// The control panel was opened; restart the music on return.
    in_menu: bool,
    sparkle_frames: u32,
    /// `viewsize` (12–20).
    viewsize: u16,
    /// `MusicMode` (F2): music on.
    music: bool,
    /// Overhead-map player blink (DS:0x3272), toggled each drawn frame.
    map_blink: bool,
    /// The bogus-mode warp prompt being typed, and the warp it produced.
    warp_prompt: Option<String>,
    pending_cheat: obc_sim::world::Cheat,
    /// Tics left of a low-health blackout (`VL_FadeOut(…, 50)` in TakeDamage
    /// 208c:05b5); the game is frozen while it runs, as the original blocks.
    blackout: u32,
    /// F10 `CP_CheckQuick`: `BACK TO DOS` asked over the game.
    quit_prompt: bool,
    /// Enhanced interpolation: player pose and actor positions at the last
    /// frame boundary, and whether it is enabled.
    prev_pose: (i32, i32, i16),
    prev_objs: Vec<Option<(i32, i32)>>,
    interpolate: bool,
    /// The transport of a network game.
    net: Option<crate::netui::NetLink>,
    /// Developer: where to put the player after each network respawn.
    pub dev_place: Option<(i32, i32, i16)>,
    /// The floors this game plays: the original's, or a mod's.
    campaign: std::sync::Arc<crate::campaign::Campaign>,
    /// `--start X,Y,DIR`: the first floor's player start (OBCEdit playtests).
    pub start_override: Option<(u8, u8, u16)>,
    /// The active mod's own art, sounds and music, converted for the game
    /// (docs/MOD_ASSETS_AND_ARENAS.md §3); `None` for the original game.
    media: Option<Arc<crate::modmedia::ModMedia>>,
}

impl Game {
    /// `NewGame(difficulty)` and the first level.
    pub fn new(ctx: &mut Ctx, difficulty: i16) -> Self {
        Self::new_at(ctx, difficulty, 0)
    }

    /// A new game starting at `mapon` (developer warp; the original's warp
    /// cheat keeps the player state instead).
    pub fn new_at(ctx: &mut Ctx, difficulty: i16, mapon: i16) -> Self {
        let mut g = Self::build(ctx, difficulty, mapon);
        g.start_level(ctx);
        g
    }

    /// A network game (game types 4–6) from the setup screen: the first
    /// NETLEVEL, the player placed by the first `NetFrame`.
    pub fn new_net(
        ctx: &mut Ctx,
        kind: i16,
        net: obc_sim::net::NetGame,
        link: crate::netui::NetLink,
    ) -> Self {
        let mut g = Self::build(ctx, kind, obc_sim::net::FIRST_NETLEVEL);
        g.world.net = Some(Box::new(net));
        g.net = Some(link);
        g.start_level(ctx);
        g
    }

    pub(crate) fn build(ctx: &mut Ctx, difficulty: i16, mapon: i16) -> Self {
        let (campaign, why) = crate::campaign::active(ctx);
        if let Some(why) = why {
            eprintln!("playing the original game: {why}");
        }
        let a = ctx.assets;
        let media = campaign
            .package
            .as_ref()
            .filter(|p| !p.assets.is_empty())
            .map(|p| Arc::new(crate::modmedia::ModMedia::load(Arc::clone(p), &a.gamepal)));
        let mut world = World::new(a.game.clone(), a.trig.clone());
        obc_sim::player::new_game(&mut world, difficulty);
        // `US_Startup` calls `US_InitRndT(true)`: the index starts at the DOS
        // clock's hundredths of a second, so each run (and Random-mode layout)
        // differs. Saves keep the index, so loads stay deterministic.
        let hundredths = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_millis() / 10);
        world.rng = obc_core::rng::Rng::seeded(hundredths as u8);
        world.gamestate.mapon = mapon;
        // A new game starts with the chosen fixes; a loaded one replaces the
        // world with its own (`from_world`).
        world.fixes = ctx.settings.game.fixes.known();
        Self {
            world,
            sound: {
                let mut s = SoundManager::new(a, ctx.settings.audio.channels);
                s.effects = crate::sound::Effects::from_setting(&ctx.settings.audio.effects);
                for (id, samples, rate) in media.iter().flat_map(|m| m.sounds.iter()) {
                    s.replace_digi(*id, Arc::clone(samples), *rate);
                }
                s
            },
            hud: Hud::default(),
            controls: Controls::default(),
            view: None,
            buffers: SceneBuffers::default(),
            splats: Vec::new(),
            shade: [build_shade_table(false), build_shade_table(true)],
            phase: Phase::FadeIn(0),
            odd_tic: false,
            damagecount: 0,
            bonuscount: 0,
            lighting: 1,
            shake_lines: 0,
            floor_mode: 2,
            show_health: false,
            crosshair: false,
            paused: false,
            in_menu: false,
            sparkle_frames: 0,
            viewsize: 18,
            music: true,
            map_blink: false,
            warp_prompt: None,
            pending_cheat: obc_sim::world::Cheat::None,
            blackout: 0,
            quit_prompt: false,
            prev_pose: (0, 0, 0),
            prev_objs: Vec::new(),
            interpolate: false,
            net: None,
            dev_place: None,
            campaign,
            start_override: None,
            media,
        }
    }

    /// True in a network game.
    pub fn is_net(&self) -> bool {
        self.net.is_some()
    }

    /// A loaded game: `GameLoop` skips `SetupGameLevel` (`loadedgame`), then
    /// `StartMusic` and the `PlayLoop` entry as usual.
    pub fn from_world(ctx: &mut Ctx, mut world: World) -> Self {
        world.loadedgame = false;
        let mut g = Self::new_at(ctx, world.gamestate.difficulty, 0);
        g.world = world;
        g.set_view_size(ctx.settings.game.viewsize);
        g.world.events.clear();
        g.start_music(ctx);
        g.world.start_play_loop();
        g.hud = Hud::default();
        g.capture_prev();
        g.phase = Phase::FadeIn(0);
        g.drain_events(ctx);
        g
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    /// For tools and tests that steer a game.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// F9 `QuickSave` (1618:1210): the slot last saved from the menu.
    fn quick_save(&mut self, ctx: &mut Ctx) {
        let Some(slot) = ctx.settings.game.quicksave_slot.map(usize::from) else {
            self.hud.message("NO CURRENT SAVE".into());
            return;
        };
        if slot >= crate::saves::SLOTS {
            self.hud.message("UNABLE TO SAVE".into());
            return;
        }
        let name = crate::saves::list(ctx)[slot]
            .as_ref()
            .map_or_else(|| format!("SLOT {}", slot + 1), |h| h.name.clone());
        match crate::saves::write(ctx, slot, &self.world, &name) {
            Ok(()) => self.hud.message(format!("SAVED {name}")),
            Err(e) => {
                eprintln!("quicksave: {e}");
                self.hud.message(format!("CANT SAVE {name}"));
            }
        }
    }

    /// `SetupGameLevel`, `StartMusic`, then the `PlayLoop` entry.
    pub fn start_level(&mut self, ctx: &mut Ctx) {
        use obc_sim::net::FIRST_NETLEVEL;
        let mut mapon = self.world.gamestate.mapon;
        // A network game with a mod's arenas plays them as slots 50 + i, and
        // after the last one the first again (docs/MOD_ASSETS_AND_ARENAS.md
        // §3.5).
        let arenas = self.campaign.arenas.len() as i16;
        let arena = (self.net.is_some() && arenas > 0).then(|| {
            if !(FIRST_NETLEVEL..FIRST_NETLEVEL + arenas).contains(&mapon) {
                mapon = FIRST_NETLEVEL;
                self.world.gamestate.mapon = mapon;
            }
            (mapon - FIRST_NETLEVEL) as usize
        });
        // The campaign's floor, or (network games, the warp cheat's floors
        // 51-60) the original map slot with its own behaviour.
        let (map, profile) = match (arena, self.campaign.level(mapon)) {
            (Some(i), _) => {
                (self.campaign.arena_map(ctx.assets, i), self.campaign.arenas[i].profile.clone())
            }
            (None, Some(l)) if self.net.is_none() => {
                (self.campaign.map(ctx.assets, mapon), l.profile.clone())
            }
            _ => (
                ctx.assets.maps.map(mapon.max(0) as usize).ok().flatten(),
                obc_sim::level::LevelProfile::stock(mapon, ctx.assets.game.cd),
            ),
        };
        let Some(mut map) = map else {
            eprintln!("map {mapon} missing; returning to the menu");
            self.phase = Phase::Sequence(After::Menu);
            return;
        };
        if let Some((x, y, dir)) = self.start_override.take() {
            move_player_start(&mut map, x, y, dir);
        }
        self.set_view_size(ctx.settings.game.viewsize);
        self.world.setup_game_level_with(&map, mapon, profile);
        if let Some(n) = self.world.net.as_deref_mut() {
            // In a mod arena, players reappear only where the start can reach.
            n.spawn_region = arena.map(|_| {
                let start = map.planes[1]
                    .iter()
                    .position(|&c| (19..=22).contains(&c))
                    .map_or((32, 32), |i| (i % 64, i / 64));
                obc_sim::level::connected_region(&map.planes[0], start)
            });
        }
        match self.media.as_deref().filter(|m| m.has_art()) {
            Some(m) => prepare_textures(
                &crate::modmedia::Layered { base: ctx.assets, media: m },
                &self.world,
            ),
            None => prepare_textures(ctx.assets, &self.world),
        }
        self.start_music(ctx);
        self.lighting = self.world.start_play_loop();
        self.hud.event = Default::default();
        self.capture_prev();
        self.damagecount = 0;
        self.bonuscount = 0;
        self.phase = Phase::FadeIn(0);
        self.drain_events(ctx);
    }

    /// `NewViewSize`: the view geometry used by the gameplay visibility pass,
    /// `GunAttack` and the splats.
    fn set_view_size(&mut self, size: u16) {
        let v = size.clamp(12, 20) & !1;
        self.viewsize = v;
        self.world.view = obc_sim::vis::ViewGeom::new(v);
        let v = v as i16;
        self.world.ply.set_view_size(v * 16, v * 8);
    }

    /// `StartMusic` (1942:0d2d): one `US_RndT` on maps ≥ 10. With CD music
    /// the current CD track instead (cdaudio).
    fn start_music(&mut self, ctx: &Ctx) {
        // A mod's music file: no song number and no random number drawn.
        if let Some(file) = self.world.level_music_file().map(str::to_owned) {
            if self.music {
                self.start_mod_music(ctx, &file);
            }
            return;
        }
        ctx.audio.send(AudioCommand::StopStream);
        let song = self.world.level_song();
        if !self.music {
            return;
        }
        if crate::cdaudio::active(ctx) {
            crate::cdaudio::start_music(ctx);
            return;
        }
        match ctx.assets.audio.music(300 + usize::from(song)) {
            Ok(m) => ctx.audio.send(AudioCommand::StartMusic(Arc::from(m))),
            Err(e) => eprintln!("music {song}: {e}"),
        }
    }

    /// Plays a mod's music file instead of any song or CD track, whatever
    /// MUSIC SOURCE says (docs/MOD_ASSETS_AND_ARENAS.md §3.4).
    fn start_mod_music(&self, ctx: &Ctx, file: &str) {
        let Some(bytes) = self.media.as_ref().and_then(|m| m.music(file)) else {
            eprintln!("music {file}: not in the mod");
            return;
        };
        ctx.audio.send(AudioCommand::StopCd);
        if file.to_ascii_lowercase().ends_with(".imf") {
            ctx.audio.send(AudioCommand::StopStream);
            match obc_formats::modassets::read_imf(&bytes) {
                Ok(m) => ctx.audio.send(AudioCommand::StartMusic(Arc::from(m))),
                Err(e) => eprintln!("music {file}: {e}"),
            }
        } else {
            ctx.audio.send(AudioCommand::StopMusic);
            ctx.audio.send(AudioCommand::PlayStream(Arc::from(bytes)));
        }
    }

    /// Records the pose and actor positions at a frame boundary.
    fn capture_prev(&mut self) {
        let p = self.world.plr();
        self.prev_pose = (p.x, p.y, p.angle);
        self.prev_objs.clear();
        self.prev_objs.extend(self.world.objs.iter().map(|o| (o.state != 0).then_some((o.x, o.y))));
    }

    fn listener(&self) -> Listener {
        let p = self.world.plr();
        Listener { x: p.x, y: p.y, angle: p.angle }
    }

    fn drain_events(&mut self, ctx: &mut Ctx) {
        let l = self.listener();
        for e in std::mem::take(&mut self.world.events) {
            match e {
                SimEvent::Sound(id) => {
                    self.sound.play(ctx.audio, id, None, l);
                    if (45..=49).contains(&id) {
                        ctx.requests.rumble = Some((0.15, 0.3, 60));
                    }
                }
                SimEvent::SoundAt { id, x, y } => self.sound.play(ctx.audio, id, Some((x, y)), l),
                SimEvent::DamageFlash(n) => {
                    self.damagecount += i32::from(n);
                    let s = (f32::from(n) / 40.0).clamp(0.2, 1.0);
                    ctx.requests.rumble = Some((s, s * 0.6, 180));
                }
                SimEvent::BonusFlash => self.bonuscount = NUMWHITESHIFTS * WHITETICS,
                // `Activity on %d` waits for the message line to be free
                // (DS:0x46A4, docs/re/NETWORK.md §4.3).
                SimEvent::Message(m)
                    if m.starts_with("Activity on") && self.hud.message.is_some() => {}
                SimEvent::Message(m) => self.hud.message(m),
                SimEvent::ItemIcon(i) => self.hud.show_item(i),
                SimEvent::AllyDied => self.hud.ally_died(),
                SimEvent::Shake(n) => {
                    let s = (f32::from(n) / 8.0).clamp(0.3, 1.0);
                    ctx.requests.rumble = Some((s, s, 400));
                }
                SimEvent::Rumble(n) => {
                    let s = f32::from(n) / 100.0;
                    ctx.requests.rumble = Some((s, s, 250));
                }
                SimEvent::Lighting(n) => self.lighting = n,
                SimEvent::Fault(f) => eprintln!("sim fault: {f}"),
                SimEvent::Blackout => self.blackout = BLACKOUT,
                SimEvent::Splat(_) | SimEvent::Hud => {}
            }
        }
    }

    /// One original frame of play.
    fn play_frame(&mut self, ctx: &mut Ctx, cmd: &TicCmd) {
        let was_full = self.world.objlistfull;
        if let Some(l) = &mut self.net {
            l.receive(&mut self.world);
        }
        let respawning = self.world.net.as_deref().is_some_and(|n| n.respawn);
        self.world.frame(cmd, FRAME_TICS);
        if let (true, Some((x, y, angle))) = (respawning, self.dev_place) {
            let p = self.world.player;
            let o = self.world.obj_mut(p);
            o.tilex = x as u16;
            o.tiley = y as u16;
            o.x = (x << 16) | 0x8000;
            o.y = (y << 16) | 0x8000;
            o.angle = angle;
        }
        if let Some(l) = &mut self.net {
            l.send(&mut self.world);
        }
        self.drain_events(ctx);
        self.sound.update(ctx.audio, self.listener());
        self.hud.frame(FRAME_TICS);
        self.palette_shifts(FRAME_TICS);
        if self.world.ply.map_overlay == 1 {
            self.map_blink = !self.map_blink;
        }
        self.sparkle_frames += 1;
        if self.sparkle_frames > 300 || (self.world.objlistfull && !was_full) {
            self.sparkle_frames = 0;
            self.hud.sparkle();
        }
        // Page flip with shake (1b4a:1420): only with TREMORS on and health ≥ 20.
        self.shake_lines = 0;
        if self.world.shake > 0 {
            if self.world.ply.tremors && self.world.gamestate.health >= 20 {
                self.shake_lines = SHAKE_LINES[(self.world.shake & 1) as usize];
            }
            self.world.shake -= 1;
        }
    }

    /// `UpdatePaletteShifts`: counters only; the palette is built when drawing.
    fn palette_shifts(&mut self, tics: i16) {
        let t = i32::from(tics);
        self.bonuscount = (self.bonuscount - t).max(0);
        self.damagecount = (self.damagecount - t).max(0);
    }

    fn base_palette(&self, a: &Assets) -> Palette {
        if self.lighting == 2 { a.darkpal } else { a.gamepal }
    }

    /// The palette for this frame: red or white shift (indices 0–15 untinted).
    fn view_palette(&self, a: &Assets) -> Palette {
        let base = self.base_palette(a);
        let red =
            if self.damagecount > 0 { (self.damagecount / 10 + 1).min(NUMREDSHIFTS) } else { 0 };
        let white = if self.bonuscount > 0 {
            (self.bonuscount / WHITETICS + 1).min(NUMWHITESHIFTS)
        } else {
            0
        };
        let (i, steps, target) = if red > 0 {
            (red, REDSTEPS, [64, 0, 0])
        } else if white > 0 {
            (white, WHITESTEPS, [64, 62, 0])
        } else {
            return base;
        };
        let mut out = base;
        for (c, o) in base.iter().zip(out.iter_mut()).skip(16) {
            for ch in 0..3 {
                let v = i32::from(c[ch] >> 2);
                let v = v + (target[ch] - v) * i / steps;
                o[ch] = expand6(v.clamp(0, 63) as u8);
            }
        }
        out
    }

    fn level_ended(&mut self, ctx: &mut Ctx) {
        match self.world.playstate {
            ex::DIED => {
                let target = self.world.start_death();
                self.drain_events(ctx);
                self.phase = Phase::Dying { target, wait: 0 };
            }
            ex::VICTORIOUS => {
                self.world.shake = 0;
                self.world.quake = 0;
                self.phase = Phase::Victory(0);
            }
            ex::WARPED => {
                // The warp keeps the player's state (GAMEFLOW §5.6); no sequence.
                ctx.audio.send(AudioCommand::StopMusic);
                if self.net.is_some() && self.world.gamestate.mapon >= obc_sim::net::FIRST_NETLEVEL
                {
                    obc_sim::net::level_done(&mut self.world);
                    self.phase = Phase::Kills(After::Restart);
                    return;
                }
                self.start_level(ctx);
            }
            ex::COMPLETED => {
                ctx.audio.send(AudioCommand::StopMusic);
                // AutoSave before `mapon` advances (GAMEFLOW §9.3).
                let w = &self.world;
                if w.gamestate.difficulty < 4
                    && self.net.is_none()
                    && let Err(e) = crate::saves::write(ctx, crate::saves::AUTOSAVE, w, "AutoSave")
                {
                    eprintln!("autosave: {e}");
                }
                self.phase = Phase::ExitFade(0);
            }
            _ => {
                ctx.audio.send(AudioCommand::StopMusic);
                self.phase = Phase::Sequence(After::Menu);
            }
        }
    }

    /// After the last floor of a campaign: the victory path, as killing Victor
    /// does (a mod's campaign; the original's floor 40 has no working exit).
    fn campaign_won(&mut self) -> bool {
        let m = self.world.gamestate.mapon;
        if self.net.is_some() || usize::try_from(m).ok() != Some(self.campaign.len()) {
            return false;
        }
        self.world.gamestate.mapon = m - 1; // the floor reached, for the scores
        // VictoryScreen, then F7, then the scores, as after Victor (GAMEFLOW §5.5).
        self.phase = Phase::Sequence(After::Victory);
        true
    }

    /// The exit sequence for the level just finished (GAMEFLOW §5.3), from
    /// the floor's profile: F2 after the sewers, F5 after floor 23, otherwise
    /// F3 (stairs, ladder) or F4 (elevator) by the exit used.
    fn exit_sequence(&self) -> Option<String> {
        match &self.world.level.sequence {
            LevelSequence::Fixed(name) => Some(name.clone()),
            LevelSequence::None => None,
            LevelSequence::Auto if self.world.ply.exitkind == 1 => Some("F4".into()),
            LevelSequence::Auto => Some("F3".into()),
        }
    }

    fn push_sequence(&mut self, ctx: &Ctx, name: &str, after: After) -> Transition {
        self.sound.stop_all(ctx.audio);
        self.phase = Phase::Sequence(after);
        match Sequence::load(ctx.assets, name) {
            Some(s) => Transition::Push(Box::new(s)),
            None => Transition::None,
        }
    }
}

impl Screen for Game {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.requests.relative_mouse = Some(true);
        self.sound.advance(1);
        // Network games keep the link alive while no frame runs.
        if let Some(l) = &mut self.net
            && (self.phase != Phase::Playing
                || self.paused
                || self.quit_prompt
                || self.warp_prompt.is_some()
                || self.blackout > 0)
        {
            l.idle();
        }
        if self.in_menu {
            // Back from the control panel: StartMusic again (GAMEFLOW §11.3).
            self.in_menu = false;
            self.set_view_size(ctx.settings.game.viewsize);
            self.sound.set_channels(ctx.settings.audio.channels);
            self.sound.effects = crate::sound::Effects::from_setting(&ctx.settings.audio.effects);
            self.start_music(ctx);
        }
        if self.blackout > 0 {
            self.blackout -= 1;
            return Transition::None;
        }
        if self.quit_prompt {
            let input = ctx.input;
            if input.pressed(obc_core::input::sc::Y)
                || input.pad_pressed(obc_core::input::pad::SOUTH)
            {
                ctx.audio.send(AudioCommand::StopMusic);
                self.sound.stop_all(ctx.audio);
                self.quit_prompt = false;
                if self.net.is_some() {
                    // EXIT TO DOS shows the KILLS table first (1618:13cb).
                    self.phase = Phase::Kills(After::Quit);
                    return Transition::None;
                }
                return Transition::Reset(Box::new(crate::menu::QuitScreen::default()));
            }
            if input.pressed(obc_core::input::sc::N)
                || input.pressed(obc_core::input::sc::ESCAPE)
                || input.pad_pressed(obc_core::input::pad::EAST)
            {
                self.quit_prompt = false;
            }
            return Transition::None;
        }
        if let Some(text) = &mut self.warp_prompt {
            // ` FLOOR: ` (1942:0b4a): two digits, Enter to warp, Esc to cancel.
            let input = ctx.input;
            for c in input.text.chars().filter(char::is_ascii_digit) {
                if text.len() < 2 {
                    text.push(c);
                }
            }
            if input.pressed(obc_core::input::sc::BACKSPACE) {
                text.pop();
            }
            if input.pressed(obc_core::input::sc::ENTER) {
                let n = text.parse::<u8>().unwrap_or(0);
                self.warp_prompt = None;
                ctx.requests.text_input = Some(false);
                self.pending_cheat = obc_sim::world::Cheat::Warp(n);
            } else if input.pressed(obc_core::input::sc::ESCAPE) {
                self.warp_prompt = None;
                ctx.requests.text_input = Some(false);
            }
            return Transition::None;
        }
        self.controls.accumulate(ctx.input, &ctx.settings.input, ctx.settings.game.nsr250);
        self.odd_tic = !self.odd_tic;
        if self.odd_tic {
            return Transition::None;
        }
        let (mut cmd, ui) =
            self.controls.build(ctx.input, &ctx.settings.input, &self.world, FRAME_TICS);
        if self.pending_cheat != obc_sim::world::Cheat::None {
            cmd.cheat = std::mem::take(&mut self.pending_cheat);
        }
        self.interpolate = ctx.settings.video.interpolate;
        self.capture_prev();

        match self.phase {
            Phase::FadeIn(t) => {
                self.phase = if t + 2 >= FADE_IN { Phase::Playing } else { Phase::FadeIn(t + 2) };
                Transition::None
            }
            Phase::Playing => {
                if self.paused {
                    if ui.pause || ui.menu || cmd.buttons.iter().any(|&b| b) {
                        self.paused = false;
                        ctx.audio.send(AudioCommand::PauseMusic(false));
                    }
                    return Transition::None;
                }
                // Esc is ignored while a tremor shakes the screen with TREMORS on
                // (DS:0x0D40 and DS:0x0D44, CheckKeys).
                if ui.menu && !(self.world.shake > 0 && self.world.ply.tremors) {
                    // Network: opening the menu is announced as a suicide (1618:061a).
                    if let Some(l) = &mut self.net {
                        let p = self.world.player;
                        obc_sim::net::player_died(&mut self.world, Some(p));
                        l.send(&mut self.world);
                    }
                    ctx.audio.send(AudioCommand::StopMusic);
                    self.sound.stop_all(ctx.audio);
                    self.in_menu = true;
                    return Transition::Push(crate::menu::in_game(Box::new(self.world.clone())));
                }
                if ui.quit {
                    self.quit_prompt = true;
                    return Transition::None;
                }
                if ui.pause {
                    self.paused = true;
                    ctx.audio.send(AudioCommand::PauseMusic(true));
                    return Transition::None;
                }
                // F1 / F2 (US_ControlPanel 1618:061a): sound and music toggles.
                if ui.sound_toggle {
                    self.sound.enabled = !self.sound.enabled;
                    if !self.sound.enabled {
                        self.sound.stop_all(ctx.audio);
                    }
                    let m = if self.sound.enabled { "SOUND  ON" } else { "SOUND  OFF" };
                    self.hud.message(m.into());
                }
                if ui.music_toggle && crate::cdaudio::active(ctx) {
                    // OBCR: F2 stops and restarts the CD track.
                    self.music = !self.music;
                    if self.music {
                        crate::cdaudio::next_track(ctx, false);
                    } else {
                        crate::cdaudio::stop(ctx);
                    }
                    self.hud.message(if self.music { "MUSIC ON" } else { "MUSIC OFF" }.into());
                } else if ui.music_toggle {
                    self.music = !self.music;
                    if self.music {
                        // SD_MusicOn resumes the current song.
                        ctx.audio.send(AudioCommand::PauseMusic(false));
                    } else {
                        ctx.audio.send(AudioCommand::PauseMusic(true));
                    }
                    self.hud.message(if self.music { "MUSIC ON" } else { "MUSIC OFF" }.into());
                }
                if crate::cdaudio::active(ctx) {
                    if ui.cd_play {
                        self.hud.message("CD : PLAY".into());
                        crate::cdaudio::next_track(ctx, false);
                    }
                    if ui.cd_stop {
                        crate::cdaudio::stop(ctx);
                        self.hud.message("CD : STOP".into());
                    }
                    if ui.cd_next {
                        self.hud.message("CD : ADVANCE TRACK".into());
                        crate::cdaudio::next_track(ctx, true);
                    }
                }
                if ui.warp && self.world.gamestate.cheatmode {
                    self.warp_prompt = Some(String::new());
                    ctx.requests.text_input = Some(true);
                    return Transition::None;
                }
                if ui.quicksave && self.world.gamestate.difficulty <= 4 && self.net.is_none() {
                    self.quick_save(ctx);
                }
                // F5 / F6 (1942:07a5, 07b2): textured floors and ceilings off / on
                // (`floormode` DS:0x33D2 = 0 / 2, kept in CONFIG.DAT).
                if ui.floor_off {
                    ctx.settings.game.floor_textures = false;
                }
                if ui.floor_on {
                    ctx.settings.game.floor_textures = true;
                }
                self.floor_mode = if ctx.settings.game.floor_textures { 2 } else { 0 };
                self.show_health ^= ui.health_overlay;
                self.crosshair ^= ui.crosshair;
                self.play_frame(ctx, &cmd);
                if self.world.level_done() {
                    self.level_ended(ctx);
                }
                Transition::None
            }
            Phase::Dying { target, wait } => {
                self.palette_shifts(FRAME_TICS);
                if let Some(t) = target
                    && !self.world.death_turn(t, FRAME_TICS)
                {
                    return Transition::None;
                }
                let wait = wait + FRAME_TICS as u32;
                if self.net.is_some() && (wait >= SCREAM_WAIT || !self.sound.digi_playing()) {
                    // GameLoop (1823:1060): respawn on the same floor, no reload.
                    self.damagecount = 0;
                    self.bonuscount = 0;
                    obc_sim::net::respawn_after_death(&mut self.world);
                    self.start_music(ctx);
                    self.lighting = self.world.start_play_loop();
                    self.capture_prev();
                    self.phase = Phase::FadeIn(0);
                    self.drain_events(ctx);
                    return Transition::None;
                }
                self.phase = if wait >= SCREAM_WAIT || !self.sound.digi_playing() {
                    // FinishPaletteShifts, lighting mode 1.
                    self.damagecount = 0;
                    self.bonuscount = 0;
                    self.lighting = 1;
                    ctx.audio.send(AudioCommand::StopMusic);
                    Phase::DeathFade(0)
                } else {
                    Phase::Dying { target: None, wait }
                };
                Transition::None
            }
            Phase::DeathFade(t) => {
                let t = t + FRAME_TICS as u32;
                if t >= DEATH_RED + DEATH_BLACK {
                    return self.push_sequence(ctx, "F6", After::Scores);
                }
                self.phase = Phase::DeathFade(t);
                Transition::None
            }
            Phase::ExitFade(t) => {
                let t = t + FRAME_TICS as u32;
                if t >= FADE_OUT && self.net.is_some() {
                    // NetLevelDone (358f:1720): forget the players, KILLS table.
                    obc_sim::net::level_done(&mut self.world);
                    self.phase = Phase::Kills(After::NextLevel);
                    return Transition::None;
                }
                if t >= FADE_OUT {
                    let Some(name) = self.exit_sequence() else {
                        self.phase = Phase::Sequence(After::NextLevel);
                        return Transition::None;
                    };
                    return self.push_sequence(ctx, &name, After::NextLevel);
                }
                self.phase = Phase::ExitFade(t);
                Transition::None
            }
            Phase::Sequence(After::NextLevel) => {
                // ClearFloorItems, mapon++, health += 35.
                self.world.advance_level();
                if self.campaign_won() {
                    return Transition::Push(Box::new(crate::menu::VictoryPic::default()));
                }
                crate::cdaudio::floor_done(ctx);
                self.start_level(ctx);
                Transition::None
            }
            Phase::Sequence(After::Menu) => Transition::Reset(crate::menu::main_menu(ctx.assets)),
            Phase::Sequence(After::Victory) => self.push_sequence(ctx, "F7", After::Scores),
            Phase::Sequence(After::Scores) => {
                // DrawHighScores(1): name entry if the score qualifies (not in bogus mode).
                let g = &self.world.gamestate;
                let entry = (!g.cheatmode).then_some((g.score, (g.mapon + 1) as u16));
                Transition::Reset(Box::new(crate::menu::HighScores::new(ctx, entry)))
            }
            Phase::Kills(after) => {
                // IN_Ack: a fresh key or button press.
                let input = ctx.input;
                let pressed = input.events.iter().any(|e| e.down && !e.repeat)
                    || input.pad.buttons & !input.pad_prev != 0;
                if !pressed {
                    return Transition::None;
                }
                match after {
                    After::Quit => {
                        return Transition::Reset(Box::new(crate::menu::QuitScreen::default()));
                    }
                    After::NextLevel => {
                        self.world.advance_level();
                        if self.campaign_won() {
                            ctx.audio.send(AudioCommand::StopMusic);
                            return Transition::Push(Box::new(crate::menu::VictoryPic::default()));
                        }
                        crate::cdaudio::floor_done(ctx);
                    }
                    _ => {}
                }
                ctx.audio.send(AudioCommand::StopMusic);
                self.start_level(ctx);
                Transition::None
            }
            Phase::Sequence(After::Restart | After::Quit) => Transition::None,
            Phase::Victory(t) => {
                let t = t + FRAME_TICS as u32;
                if t >= 1000 || ui.menu || cmd.buttons.iter().any(|&b| b) {
                    ctx.audio.send(AudioCommand::StopMusic);
                    // VictoryScreen, then F7 (not shipped), then the scores (GAMEFLOW §5.5).
                    self.sound.stop_all(ctx.audio);
                    self.phase = Phase::Sequence(After::Victory);
                    return Transition::Push(Box::new(crate::menu::VictoryPic::default()));
                }
                self.phase = Phase::Victory(t);
                Transition::None
            }
        }
    }

    fn game_mut(&mut self) -> Option<&mut Game> {
        Some(self)
    }

    fn name(&self) -> &'static str {
        "game"
    }

    fn draw(&mut self, a: &Assets, out: &mut Frame) {
        if let (Phase::Kills(_), Some(n)) = (self.phase, self.world.net.as_deref()) {
            out.frame.clear(0);
            out.layer_key = None;
            crate::netui::draw_kills(out.layer, a, n);
            out.palette = a.gamepal;
            out.scroll = 0;
            return;
        }
        let p = out.presentation;
        let l = layout(p, self.viewsize);
        if self.view.as_ref().is_none_or(|(vl, _)| *vl != l) {
            self.view = Some((l, View3D::new(l.cfg)));
        }
        tile_background(out.frame, a, p.scale);
        let w = &self.world;
        // Time since the last frame boundary, in frames (Enhanced only).
        let lerp = (self.interpolate && !p.is_classic())
            .then(|| ((f32::from(u8::from(self.odd_tic)) + out.alpha) / 2.0).clamp(0.0, 1.0));
        self.buffers.fill(w, lerp.map(|t| (self.prev_objs.as_slice(), t)));
        let plr = w.plr();
        let (px, py, fine) = match lerp {
            Some(t) => {
                let (x0, y0, a0) = self.prev_pose;
                let blend = |a: i32, b: i32| a + ((b - a) as f32 * t) as i32;
                let da = (i32::from(plr.angle) - i32::from(a0) + 540).rem_euclid(360) - 180;
                let fine = (i32::from(a0) * 10 + (da as f32 * 10.0 * t) as i32).rem_euclid(3600);
                (blend(x0, plr.x), blend(y0, plr.y), Some(fine))
            }
            None => (plr.x, plr.y, None),
        };
        let scene = Scene {
            tiles: &w.tilemap,
            info: &w.plane1,
            plane0: &w.plane0,
            doors: &self.buffers.doors,
            pwall_pos: w.pwall.pos,
            floor: floor_params(&a.game.levels, w, self.floor_mode),
            shade_table: &self.shade[usize::from(w.dark)],
            shade_bias: 2 * i32::from(w.ply.lightbits),
            player_x: px,
            player_y: py,
            player_angle: fine.map_or(i32::from(plr.angle), |f| f / 10),
            player_fine_angle: fine,
            sprites: &self.buffers.sprites,
            weapon: weapon_view(w),
            smooth_floors: out.smooth_floors,
        };
        self.splats.clear();
        for s in &w.ply.splats[..usize::from(w.ply.splatcount)] {
            let (shape, height) = match s.kind {
                2 => (640 + s.shape as u16, i32::from(s.y) >> 1),
                1 => (648 + s.shape as u16, i32::from(s.y)),
                _ => continue,
            };
            self.splats.push(SplatView { shape, x: i32::from(s.x), height });
        }
        if let Some((_, view)) = &mut self.view {
            // A mod's replaced walls and sprites over the game's.
            let layered = self
                .media
                .as_deref()
                .filter(|m| m.has_art())
                .map(|m| crate::modmedia::Layered { base: a, media: m });
            let tex: &dyn obc_render::view3d::Textures = match &layered {
                Some(t) => t,
                None => a,
            };
            view.render(out.frame, l.x, l.y, &scene, tex);
            view.draw_splats(out.frame, l.x, l.y, &self.splats, tex);
        }
        out.layer.clear(0xFF);
        out.layer_key = Some(0xFF);
        out.layer_hud = true;
        let (vx, vy) = (i32::from(160 - self.viewsize * 8), i32::from(80 - self.viewsize * 4));
        let (vw, vh) = (i32::from(self.viewsize) * 16, i32::from(self.viewsize) * 8);
        if self.crosshair {
            draw_crosshair(out.layer, vx + vw / 2, vy + vh / 2);
        }
        if self.show_health {
            draw_health(out.layer, a, w.gamestate.health, vx + vw, vy);
        }
        if w.ply.map_overlay == 1 {
            draw_overhead_map(out.layer, w, self.viewsize, self.map_blink);
        }
        self.hud.draw(out.layer, a, w);
        self.hud.draw_message(out.layer, a, vx, vy);
        if self.paused {
            draw_box(out.layer, a, "PAUSE");
        }
        if let Some(t) = &self.warp_prompt {
            draw_box(out.layer, a, &format!(" FLOOR: {t}_"));
        }
        if self.quit_prompt {
            draw_box(out.layer, a, "BACK TO DOS Y/N");
        }

        let mut pal = self.view_palette(a);
        if self.blackout > 0 {
            pal = faded(&pal, (BLACKOUT - self.blackout) as i32, BLACKOUT as i32, [0, 0, 0]);
        }
        out.palette = match self.phase {
            Phase::FadeIn(t) => faded(&pal, (FADE_IN - t) as i32, FADE_IN as i32, [0, 0, 0]),
            Phase::ExitFade(t) => faded(&pal, t as i32, FADE_OUT as i32, [0, 0, 0]),
            Phase::DeathFade(t) if t < DEATH_RED => {
                // VL_FadeOut to (10, 0, 0) on the 6-bit scale.
                faded(&pal, t as i32, DEATH_RED as i32, [40, 0, 0])
            }
            Phase::DeathFade(t) => {
                let red = faded(&pal, 1, 1, [40, 0, 0]);
                faded(&red, (t - DEATH_RED) as i32, DEATH_BLACK as i32, [0, 0, 0])
            }
            Phase::Sequence(_) => faded(&pal, 1, 1, [0, 0, 0]),
            _ => pal,
        };
        out.scroll = self.shake_lines;
    }
}

/// Starts a game on its first tic (for `--play`, which has no `Ctx` at startup).
pub struct Boot {
    pub difficulty: i16,
    pub mapon: i16,
    /// `--start X,Y,DIR`: where the player starts on that floor.
    pub start: Option<(u8, u8, u16)>,
}

impl Screen for Boot {
    fn tick(&mut self, ctx: &mut Ctx) -> Transition {
        let mut g = Game::build(ctx, self.difficulty, self.mapon);
        g.start_override = self.start;
        g.start_level(ctx);
        Transition::Replace(Box::new(g))
    }

    fn draw(&mut self, _a: &Assets, out: &mut Frame) {
        out.frame.clear(0);
        out.layer.clear(0);
    }
}

/// Builds the xBRZ versions (if enabled) of the textures a new floor shows
/// first: its walls and doors, floors and ceilings, statics, the actors'
/// current frames and the hand weapons. Other frames follow on first use.
fn prepare_textures(a: &(dyn obc_render::view3d::Textures + Sync), w: &World) {
    if a.hires() < 2 {
        return;
    }
    let mut walls: Vec<u16> = w
        .tilemap
        .iter()
        .filter(|&&t| t != 0)
        .map(|&t| u16::from(t & 0x7F).wrapping_sub(1))
        .collect();
    walls.extend(w.doors.iter().map(|d| d.kind));
    walls.push(130); // door frame
    let flats: Vec<u16> = w
        .floor_sets
        .iter()
        .flat_map(|&(f, c)| [f, c])
        .map(|p| u16::from(p).wrapping_sub(1))
        .collect();
    let mut sprites: Vec<u16> =
        w.statics.iter().filter(|s| s.shape >= 0).map(|s| s.shape as u16).collect();
    let mut cur = w.plr().next;
    while let Some(id) = cur {
        let o = w.obj(id);
        cur = o.next;
        if let Some(st) = w.data.state(o.state).filter(|s| s.shape > 0) {
            let base = st.shape as u16;
            sprites.extend(base..base + if st.rotate != 0 { 8 } else { 1 });
        }
    }
    sprites.extend(544..624); // both halves of every hand weapon
    for v in [&mut walls, &mut sprites] {
        v.sort_unstable();
        v.dedup();
    }
    crate::assets::prepare_upscaled(a, &walls, &flats, &sprites);
}

/// The Tab overhead map (`2f4d:0d31`, RENDERER.md §9.2), one pixel per tile
/// at absolute page coordinates, drawn over the view without a background.
fn draw_overhead_map(fb: &mut Framebuffer, w: &World, viewsize: u16, blink: bool) {
    let (vw, vh) = (i32::from(viewsize) * 16, i32::from(viewsize) * 8);
    let (x0, y0) = ((320 - vw) / 2 + 4, (200 - vh) / 2 + 4);
    // Frame: a 64×64 outline in 0x0C.
    fb.bar(x0, y0, 64, 1, 0x0C);
    fb.bar(x0, y0 + 63, 64, 1, 0x0C);
    fb.bar(x0, y0, 1, 64, 0x0C);
    fb.bar(x0 + 63, y0, 1, 64, 0x0C);
    // Walls: a 24×24 window around the player, columns from a multiple of 8.
    let p = w.plr();
    let window = |t: u16| -> i32 {
        let c = (i32::from(t as i16) - 12).max(0);
        if c + 24 > 64 { 40 } else { c }
    };
    let (c0, r0) = (window(p.tilex) & !7, window(p.tiley));
    for ty in r0..r0 + 24 {
        let row = w.automap.get(ty as usize).copied().unwrap_or(0);
        for tx in c0..(c0 + 24).min(64) {
            if row & (1 << tx) != 0 {
                fb.put(x0 + tx, y0 + ty, 0x0C);
            }
        }
    }
    if w.gamestate.difficulty < 4 {
        // ThermoSensor: every actor that ran this frame, inside the window.
        if w.gamestate.cheatmode || w.thermosensor {
            for &v in &w.actorpos {
                let (tx, ty) = (i32::from(v >> 8), i32::from(v & 0xFF));
                if v != 0 && (c0..c0 + 24).contains(&tx) && (r0..r0 + 24).contains(&ty) {
                    fb.put(x0 + tx, y0 + ty, 1);
                }
            }
        }
        // Allies, anywhere in the box.
        for id in w.obj_ids() {
            let o = w.obj(id);
            if id != w.player && o.obclass == obc_sim::world::class::ALLY {
                fb.put(x0 + i32::from(o.tilex as i16), y0 + i32::from(o.tiley as i16), 2);
            }
        }
    }
    let c = if blink { 0xFC } else { 0x0F };
    fb.put(x0 + i32::from(p.tilex as i16), y0 + i32::from(p.tiley as i16), c);
}

/// The C crosshair (30d6:07d0): eleven dots every other pixel across and down
/// through the view centre (`viewwidth/2`, `viewheight/2`), colour 0xE1.
fn draw_crosshair(fb: &mut Framebuffer, cx: i32, cy: i32) {
    for i in (0..22).step_by(2) {
        fb.put(cx + i - 10, cy, 0xE1);
        fb.put(cx, cy + i - 10, 0xE1);
    }
}

/// The H health readout (30d6:0779; positions from `SetViewSize` 1497:1143):
/// `%3d` in font 1 at (viewwidth − 27, 0) with a black shadow, in the font's
/// own colours, or flat 0xFE below 50; the health icon (pic 24) at
/// (viewwidth − 40, 2). `right`/`top` are the view's right edge and top.
fn draw_health(fb: &mut Framebuffer, a: &Assets, health: i16, right: i32, top: i32) {
    use obc_render::draw2d::{draw_text, draw_text_native};
    if let Some(font) = a.fonts.get(1) {
        let text = format!("{health:3}");
        let (x, y) = (right - 27, top);
        draw_text(fb, font, text.as_bytes(), x + 1, y + 1, 0);
        if health < 50 {
            draw_text(fb, font, text.as_bytes(), x, y, 0xFE);
        } else {
            draw_text_native(fb, font, text.as_bytes(), x, y);
        }
    }
    if let Some(icon) = a.pics.get(24) {
        obc_render::draw2d::draw_pic(fb, icon, right - 40, top + 2);
    }
}

/// A centred message box (`US_CenterWindow` + print), used for PAUSE.
pub fn draw_box(fb: &mut Framebuffer, a: &Assets, text: &str) {
    let Some(font) = a.fonts.first() else { return };
    let w = obc_render::draw2d::text_width(font, text.as_bytes()) + 16;
    let h = i32::from(font.height) + 10;
    let (x, y) = (160 - w / 2, 80 - h / 2);
    fb.bar(x - 1, y - 1, w + 2, h + 2, 0);
    fb.bar(x, y, w, h, 0x1C);
    obc_render::draw2d::draw_text(fb, font, text.as_bytes(), x + 8, y + 5, 0x0F);
}

/// Moves a map's player start to `(x, y)` facing compass `dir` (0 N .. 3 W),
/// removing the others: `--start`, for OBCEdit's "play from here".
fn move_player_start(map: &mut obc_formats::maps::Map, x: u8, y: u8, dir: u16) {
    let w = usize::from(map.width);
    for c in &mut map.planes[1] {
        if (19..=22).contains(c) {
            *c = 18;
        }
    }
    if let Some(c) = map.planes[1].get_mut(usize::from(y) * w + usize::from(x)) {
        *c = 19 + dir.min(3);
    }
}
