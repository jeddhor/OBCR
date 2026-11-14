//! Data-gated (CD release): the CD-music logic of `BCCD.EXE`
//! (docs/re/CD_RELEASE.md §4) over a real mixer, with short synthetic tracks
//! standing in for the ripped ones.

use std::sync::atomic::Ordering;

use obc_audio::Mixer;
use obc_core::input::InputState;
use obc_formats::DataDir;
use obc_ui::assets::Assets;
use obc_ui::cdaudio;
use obc_ui::screen::{Ctx, Requests};
use obc_ui::settings::Settings;

/// The CD data plus four 0.1 s tracks, in a fresh temporary folder.
fn cd_folder() -> Option<std::path::PathBuf> {
    let src = obc_testdata::cd_data_dir()?;
    let dir = std::env::temp_dir().join(format!("obcr-cd-music-{}", std::process::id()));
    std::fs::create_dir_all(&dir).ok()?;
    for e in std::fs::read_dir(src).ok()?.flatten() {
        std::fs::copy(e.path(), dir.join(e.file_name())).ok()?;
    }
    for n in 1..=4i16 {
        let pcm: Vec<i16> = (0..4410).flat_map(|i| [(i % 100) as i16 * 50 * n, 0]).collect();
        let qoa = obc_formats::qoa::encode(&pcm, 2, 44_100).ok()?;
        std::fs::write(dir.join(cdaudio::track_file(n)), qoa).ok()?;
    }
    Some(dir)
}

#[test]
fn cd_music_follows_the_cd_release() {
    let Some(dir) = cd_folder() else {
        eprintln!("SKIPPED: CD release data not found (set OBC_CD_DATA_DIR)");
        return;
    };
    let assets = Assets::load(DataDir::open(&dir).unwrap()).unwrap();
    assert_eq!(assets.release.release.id, "OBC-CD");
    assert!(assets.has_cd_audio() && assets.game.cd);
    let (mut mixer, audio) = Mixer::new(48_000);
    let mut settings = Settings::default();
    let input = InputState::default();
    let saves = std::env::temp_dir();
    let mut requests = Requests::default();
    let ctx = Ctx {
        assets: &assets,
        audio: &audio,
        input: &input,
        settings: &mut settings,
        requests: &mut requests,
        save_dir: &saves,
    };
    let cd = audio.cd.clone();
    let track = || cd.track.load(Ordering::Relaxed);
    let playing = || cd.playing.load(Ordering::Relaxed);
    let run_out = |mixer: &mut Mixer| {
        let mut buf = vec![0.0; 2 * 12_000];
        mixer.fill(&mut buf);
    };
    assert!(cdaudio::active(&ctx));

    // StartMusic plays track 1, and only once.
    cdaudio::start_music(&ctx);
    assert!(cd.busy() && playing() && track() == 1);
    run_out(&mut mixer);
    assert!(!cd.busy() && playing(), "the track ended; the flag stays");
    cdaudio::start_music(&ctx);
    assert!(!cd.busy(), "StartMusic does not restart a track that played");

    // Original CD TRACK END: silence until the floor is done, then track 1 again.
    ctx.settings.audio.cd_next_track = false;
    cdaudio::service(&ctx);
    assert!(!cd.busy());
    cdaudio::floor_done(&ctx);
    assert!(cd.busy() && track() == 1);
    run_out(&mut mixer);

    // Enhanced: the next track follows, 4 wrapping to 1.
    ctx.settings.audio.cd_next_track = true;
    cdaudio::service(&ctx);
    assert!(cd.busy() && track() == 2);
    cd.track.store(4, Ordering::Relaxed);
    run_out(&mut mixer);
    cdaudio::service(&ctx);
    assert_eq!(track(), 1);

    // Alt+S stops for good (no restart while stopped); Alt+P plays the current one.
    cdaudio::stop(&ctx);
    run_out(&mut mixer);
    cdaudio::service(&ctx);
    assert!(!cd.busy() && !playing());
    cdaudio::next_track(&ctx, false);
    assert!(cd.busy() && track() == 1);

    // Alt+F past track 3: CD_NextTrack plays 1 and makes 2 current.
    cd.track.store(4, Ordering::Relaxed);
    cdaudio::next_track(&ctx, true);
    assert_eq!(track(), 2);
    cd.track.store(2, Ordering::Relaxed);
    cdaudio::next_track(&ctx, true);
    assert_eq!(track(), 3);

    // MUSIC SOURCE = AdLib turns all of it off.
    ctx.settings.audio.music_source = "adlib".into();
    cdaudio::source_changed(&ctx);
    run_out(&mut mixer);
    assert!(!cdaudio::active(&ctx) && !cd.busy());
    cdaudio::start_music(&ctx);
    assert!(!cd.busy());
    std::fs::remove_dir_all(&dir).ok();
}
