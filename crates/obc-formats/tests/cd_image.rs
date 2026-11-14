//! Data-gated: the Steam/GOG BIN/CUE image (`OBC_CD_IMAGE`, or the `.cue`
//! in `../CD`). Identifies as OBC-CD straight off the disc, and its audio
//! tracks survive QOA.

use obc_formats::disc::Disc;
use obc_formats::qoa;
use obc_formats::release;

#[test]
fn reads_the_cd_image() {
    let Some(cue) = obc_testdata::cd_image() else {
        eprintln!("SKIPPED: CD image not found (set OBC_CD_IMAGE)");
        return;
    };
    let disc = Disc::open(&cue).unwrap();
    let id = release::identify_with(|n| disc.read(n).ok());
    assert_eq!(id.release.id, "OBC-CD");
    assert!(id.is_complete() && id.has_sequences());
    // The .bin opens through its .cue too.
    let bin = cue.with_extension("bin");
    if bin.is_file() {
        assert_eq!(Disc::open(&bin).unwrap().names().count(), disc.names().count());
    }

    // Four music tracks, 2..=5, lengths from the cue sheet (06:12:52, 14:01:68, ...).
    let t = disc.audio_tracks();
    assert_eq!(t.iter().map(|t| t.number).collect::<Vec<_>>(), [2, 3, 4, 5]);
    assert_eq!(t[0].sectors, (14 * 60 + 1) * 75 + 68 - ((6 * 60 + 12) * 75 + 52));
    assert!(t.iter().all(|t| (300.0..500.0).contains(&t.seconds())));

    // Five seconds of track 2 through QOA keep a sane signal-to-noise ratio.
    let mut pcm = Vec::new();
    disc.read_audio(&t[0], |s| {
        pcm.extend_from_slice(s);
        pcm.len() < 44_100 * 2 * 5
    })
    .unwrap();
    let (info, out) = qoa::decode(&qoa::encode(&pcm, 2, 44_100).unwrap()).unwrap();
    assert_eq!((info.channels, out.len()), (2, pcm.len()));
    let sig: f64 = pcm.iter().map(|&s| f64::from(s).powi(2)).sum();
    let noise: f64 =
        pcm.iter().zip(&out).map(|(&a, &b)| (f64::from(a) - f64::from(b)).powi(2)).sum();
    assert!(sig > 0.0, "track 2 starts with music, not silence");
    let snr = 10.0 * (sig / noise.max(1.0)).log10();
    assert!(snr > 20.0, "SNR {snr:.1} dB");
}
