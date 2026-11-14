//! The CD release's Red Book music (docs/re/CD_RELEASE.md §4), played from
//! the tracks the installer ripped off the disc image (`TRACK02.QOA` ..
//! `TRACK05.QOA` in the data directory, ADR-022).
//!
//! `BCCD.EXE` with `cdmusic` plays no FM music. It keeps a current track,
//! 1..=4 (disc tracks 2..5, starting at 1), and a `playing` flag:
//!
//! - `StartMusic` (level start, back from the menu) plays the current track
//!   unless one was started and not stopped;
//! - after a floor is finished, the current track is played again if the
//!   drive is idle, so a track that ended stays silent until then;
//! - `F1`, `F5` and `F7` stop the CD first;
//! - Alt+P / Alt+S / Alt+F play, stop and advance (`CD_NextTrack`).
//!
//! With MUSIC SOURCE set to CD audio, OBCR does the same. CD TRACK END
//! (Enhanced, on by default) goes on to the next track when one ends
//! instead of waiting for the next floor.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use obc_audio::AudioCommand;

use crate::screen::Ctx;

/// Tracks the game cycles through.
pub const TRACKS: i16 = 4;

/// CD music is on: tracks were ripped and MUSIC SOURCE is CD audio
/// (the original's `cdmusic`, DS:0x2CFA).
pub fn active(ctx: &Ctx) -> bool {
    ctx.assets.has_cd_audio() && ctx.settings.audio.music_source != "adlib"
}

/// `CD_Play(n)` (cd 2b64:193f): track 0 means 1; sets `playing`.
pub fn play(ctx: &Ctx, n: i16) {
    if !active(ctx) {
        return;
    }
    let n = if n == 0 { 1 } else { n };
    stop(ctx);
    if let Some(data) = ctx.assets.cd_track(n) {
        ctx.audio.cd.mark_busy();
        ctx.audio.send(AudioCommand::PlayCd(data));
    }
    ctx.audio.cd.playing.store(true, Ordering::Relaxed);
}

/// `CD_Stop` (cd 2b64:19a3): clears `playing`.
pub fn stop(ctx: &Ctx) {
    if !ctx.assets.has_cd_audio() {
        return;
    }
    ctx.audio.cd.playing.store(false, Ordering::Relaxed);
    ctx.audio.send(AudioCommand::StopCd);
}

/// `CD_NextTrack(advance)` (cd 2b64:18f7). Outside 1..=3 it plays track 1
/// and makes 2 current (so the next restart plays track 2); otherwise it
/// advances first if asked and plays the current track.
pub fn next_track(ctx: &Ctx, advance: bool) {
    if !active(ctx) {
        return;
    }
    let cd = &ctx.audio.cd;
    let t = cd.track.load(Ordering::Relaxed);
    if !(1..=3).contains(&t) {
        cd.track.store(2, Ordering::Relaxed);
        play(ctx, 1);
        return;
    }
    let t = if advance { t + 1 } else { t };
    cd.track.store(t, Ordering::Relaxed);
    play(ctx, t);
}

/// `StartMusic` with `cdmusic` (cd 1a50:0041).
pub fn start_music(ctx: &Ctx) {
    if active(ctx) && !ctx.audio.cd.playing.load(Ordering::Relaxed) {
        play(ctx, ctx.audio.cd.track.load(Ordering::Relaxed));
    }
}

/// `GameLoop` after a finished floor (cd:1946:0134 and 01be-01e0): the
/// current track again if none was started or the drive is idle.
pub fn floor_done(ctx: &Ctx) {
    if active(ctx) && (!ctx.audio.cd.playing.load(Ordering::Relaxed) || !ctx.audio.cd.busy()) {
        play(ctx, ctx.audio.cd.track.load(Ordering::Relaxed));
    }
}

/// Every tic (Enhanced CD TRACK END = NEXT TRACK): a track that ended while
/// playing gives way to the next, 4 wrapping to 1.
pub fn service(ctx: &Ctx) {
    let cd = &ctx.audio.cd;
    if active(ctx)
        && ctx.settings.audio.cd_next_track
        && cd.playing.load(Ordering::Relaxed)
        && !cd.busy()
    {
        let t = cd.track.load(Ordering::Relaxed).clamp(1, TRACKS) % TRACKS + 1;
        cd.track.store(t, Ordering::Relaxed);
        play(ctx, t);
    }
}

/// The sequences that stop the CD first (cd:33bd:0086): the CD-only ones.
pub fn stops_for_sequence(name: &str) -> bool {
    matches!(name, "F1" | "F5" | "F7")
}

/// Turning MUSIC SOURCE away from CD audio.
pub fn source_changed(ctx: &Ctx) {
    if active(ctx) {
        ctx.audio.send(AudioCommand::StopMusic);
    } else if ctx.assets.has_cd_audio() {
        ctx.audio.cd.playing.store(false, Ordering::Relaxed);
        ctx.audio.send(AudioCommand::StopCd);
    }
}

/// A track's bytes, shared with the mixer.
pub(crate) fn load(data: &obc_formats::DataDir, n: i16) -> Option<Arc<[u8]>> {
    data.read(&track_file(n)).ok().map(Arc::from)
}

/// File name of track `n` (1..=4): disc track `n + 1`.
pub fn track_file(n: i16) -> String {
    format!("TRACK{:02}.QOA", n + 1)
}
