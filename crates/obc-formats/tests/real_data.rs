//! Data-gated tests against release OBC-A (docs/re/DATA_FORMATS.md).

use obc_formats::DataDir;
use obc_formats::audio::{AudioArchive, parse_audiomus};
use obc_formats::config::parse_high_scores;
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, MapArchive};
use obc_formats::release;
use obc_formats::vgagraph::{Font, GraphArchive, deplanarize};
use obc_formats::vswap::PageFile;

fn data() -> Option<DataDir> {
    obc_testdata::data_dir().map(|d| DataDir::open(d).expect("data dir readable"))
}

macro_rules! data_or_skip {
    () => {
        match data() {
            Some(d) => d,
            None => {
                eprintln!("SKIPPED: original game data not found (set OBC_DATA_DIR)");
                return;
            }
        }
    };
}

#[test]
fn release_is_identified() {
    let d = data_or_skip!();
    let id = release::identify(&d);
    assert_eq!(id.release.id, "OBC-A");
    assert!(id.is_complete());
}

#[test]
fn page_file_counts() {
    let d = data_or_skip!();
    let pf = PageFile::parse(d.read("GFXTILES.BC").unwrap()).unwrap();
    assert_eq!((pf.num_pages(), pf.num_walls(), pf.num_sprites()), (937, 136, 680));
    for i in 0..pf.num_walls() {
        pf.wall(i).unwrap();
    }
    for i in 0..pf.num_sprites() {
        pf.sprite(i).unwrap();
    }
    assert_eq!(pf.digi_sounds().unwrap().len(), 46);
}

#[test]
fn every_map_decodes() {
    let d = data_or_skip!();
    let maps = MapArchive::parse(
        &d.read("MAPHEAD.BC").unwrap(),
        d.read("MAPTEMP.BC").unwrap(),
        Compression::Rlew,
    )
    .unwrap();
    assert_eq!(maps.rlew_tag(), 0xABCD);
    let slots: Vec<usize> = maps.used_slots().collect();
    assert_eq!(slots, (0..61).collect::<Vec<_>>());
    for s in slots {
        let m = maps.map(s).unwrap().unwrap();
        assert_eq!((m.width, m.height), (64, 64));
        assert!(m.planes[2].iter().all(|&v| v == 0), "plane 2 of map {s} not empty");
    }
    assert_eq!(maps.name(0).unwrap().unwrap(), "SEWER LEVEL 1");
    assert_eq!(maps.name(39).unwrap().unwrap(), "BLDG2 TOP");
}

#[test]
fn graphics_decode() {
    let d = data_or_skip!();
    let ga = GraphArchive::parse(
        &d.read("VGAHEAD.BC").unwrap(),
        &d.read("VGADICT.BC").unwrap(),
        d.read("VGAGRAPH.BC").unwrap(),
    )
    .unwrap();
    assert_eq!(ga.num_chunks(), 58);
    let table = ga.pic_table().unwrap();
    assert_eq!(table.len(), 53);
    for (i, &(w, h)) in table.iter().enumerate() {
        deplanarize(&ga.chunk(3 + i).unwrap().unwrap(), w, h).unwrap();
    }
    assert_eq!(Font::parse(&ga.chunk(1).unwrap().unwrap()).unwrap().height, 13);
    assert_eq!(Font::parse(&ga.chunk(2).unwrap().unwrap()).unwrap().height, 10);
}

#[test]
fn audio_decodes() {
    let d = data_or_skip!();
    let a =
        AudioArchive::parse(&d.read("AUDIOHED.BC").unwrap(), d.read("AUDIOT.BC").unwrap()).unwrap();
    assert_eq!(a.num_chunks(), 310);
    let first = a.pc_sound(0).unwrap();
    assert_eq!((first.data.len(), first.priority), (109, 99));
    assert_eq!(parse_audiomus(&d.read("AUDIOMUS.BC").unwrap()).unwrap().len(), 99);
}

#[test]
fn palettes_and_scores_from_exe() {
    let d = data_or_skip!();
    let exe = MzExe::parse(d.read("COUNT.EXE").unwrap()).unwrap();
    assert_eq!(exe.header_bytes(), 0x3400);
    exe.palette(release::OBC_A.gamepal_offset).unwrap();
    exe.palette(0x2E3B0).unwrap();
    let scores = parse_high_scores(exe.file(0x383E4, 7 * 28).unwrap()).unwrap();
    assert_eq!(scores[0].name, "Hank");
    assert_eq!(scores[0].score, 11200);
    assert_eq!(scores[6].name, "Arnold");
}

#[test]
fn sequences_decode() {
    let d = data_or_skip!();
    for (name, frames) in [("F2.BC", 31), ("F3.BC", 30), ("F4.BC", 54), ("F6.BC", 34)] {
        let Ok(bytes) = d.read(name) else { continue };
        let anim = obc_formats::flc::decode(&bytes).unwrap();
        assert_eq!(anim.frames.len(), frames, "{name}");
    }
}
