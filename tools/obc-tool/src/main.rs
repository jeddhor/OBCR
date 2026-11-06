//! `obc-tool`: inspect, validate and export Operation Body Count data.
//!
//! Exports derived from the original data belong in `.local/exports/` and must
//! never be committed (AGENT_GUIDE §10.3).

mod out;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use obc_formats::audio::{AudioArchive, parse_audiomus};
use obc_formats::exe::MzExe;
use obc_formats::maps::{Compression, MapArchive};
use obc_formats::release::{self, FileStatus};
use obc_formats::vgagraph::{Font, GraphArchive, deplanarize};
use obc_formats::vswap::{PageFile, SHAPE_SIZE};
use obc_formats::{DataDir, datadir, flc};

type Palette = [[u8; 3]; 256];

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Game data directory (default: discovered as in DESIGN §6.7).
    #[arg(long, global = true)]
    data: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Identify the data release and check every file.
    Info,
    /// Export walls, sprites and digitized sounds from GFXTILES.
    Vswap {
        #[arg(long, default_value = ".local/exports/vswap")]
        out: PathBuf,
    },
    /// Map tools.
    Maps {
        #[command(subcommand)]
        command: MapsCommand,
    },
    /// Export pictures and fonts from VGAGRAPH.
    Vga {
        #[arg(long, default_value = ".local/exports/vga")]
        out: PathBuf,
        /// Use the 6-bit palette at this COUNT.EXE file offset instead of gamepal.
        #[arg(long, value_parser = parse_hex)]
        palette_offset: Option<usize>,
    },
    /// Export AUDIOT sounds (PC speaker as WAV) and the AUDIOMUS digitized sounds.
    Audio {
        #[arg(long, default_value = ".local/exports/audio")]
        out: PathBuf,
        /// Sample rate assumed for AUDIOMUS (Corridor 7 uses 9009 Hz).
        #[arg(long, default_value_t = 9009)]
        audiomus_rate: u32,
        /// Sample rate assumed for GFXTILES digitized sounds.
        #[arg(long, default_value_t = 7042)]
        vswap_rate: u32,
    },
    /// Export rendered-sequence (FLC) frames as PNG.
    Flc {
        #[arg(long, default_value = ".local/exports/flc")]
        out: PathBuf,
    },
    /// Write the game palette as a 16×16 swatch PNG.
    Pal {
        #[arg(long, default_value = ".local/exports/palette.png")]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum MapsCommand {
    /// List map slots and names.
    List,
    /// Histogram of plane values over all maps.
    Stats {
        #[arg(long, default_value_t = 0)]
        plane: usize,
    },
    /// Every map and tile where a plane value occurs.
    Find {
        #[arg(long)]
        plane: usize,
        #[arg(long)]
        value: u16,
    },
    /// Print a plane as a grid of numbers.
    Dump {
        slot: usize,
        #[arg(long, default_value_t = 0)]
        plane: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = datadir::discover(cli.data.as_deref(), None)
        .context("no Operation Body Count data found (use --data or OBC_DATA_DIR)")?;
    let data = DataDir::open(&dir)?;
    match cli.command {
        Command::Info => info(&data),
        Command::Vswap { out } => vswap(&data, &out),
        Command::Maps { command } => maps(&data, command),
        Command::Vga { out, palette_offset } => vga(&data, &out, palette_offset),
        Command::Audio { out, audiomus_rate, vswap_rate } => {
            audio(&data, &out, audiomus_rate, vswap_rate)
        }
        Command::Flc { out } => flc_export(&data, &out),
        Command::Pal { out } => {
            let pal = palette(&data)?;
            let mut rgba = Vec::with_capacity(256 * 4);
            for c in pal {
                rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
            out::png_rgba(&out, 16, 16, &rgba)
        }
    }
}

fn info(data: &DataDir) -> Result<()> {
    println!("data directory: {}", data.root().display());
    let id = release::identify(data);
    println!("release: {} — {}", id.release.id, id.release.description);
    for c in &id.checks {
        let status = match &c.status {
            FileStatus::Ok => "ok".to_owned(),
            FileStatus::Missing => "MISSING".to_owned(),
            FileStatus::Present => "present (user state, not hashed)".to_owned(),
            FileStatus::Mismatch { size, sha256 } => {
                format!("MISMATCH size {size} sha256 {sha256}")
            }
        };
        println!("  {:<13} {:<10} {}", c.file.name, format!("{:?}", c.file.role), status);
    }
    println!(
        "complete: {}   rendered sequences: {}",
        if id.is_complete() { "yes" } else { "NO" },
        if id.has_sequences() { "present" } else { "absent" }
    );
    Ok(())
}

fn parse_hex(s: &str) -> Result<usize, std::num::ParseIntError> {
    usize::from_str_radix(s.trim_start_matches("0x"), 16)
}

fn palette(data: &DataDir) -> Result<Palette> {
    let id = release::identify(data);
    let exe = MzExe::parse(data.read("COUNT.EXE")?)?;
    Ok(exe.palette(id.release.gamepal_offset)?)
}

fn vswap(data: &DataDir, out: &Path) -> Result<()> {
    let pal = palette(data)?;
    let pf = PageFile::parse(data.read("GFXTILES.BC")?)?;
    println!("{} pages: {} walls, {} sprites", pf.num_pages(), pf.num_walls(), pf.num_sprites());
    let transparent = [0x40, 0x00, 0x40, 0xFF];
    let mut walls = Vec::new();
    for i in 0..pf.num_walls() {
        let wall = pf.wall(i)?;
        let img = column_major_rgba(wall, None, &pal, transparent);
        out::png_rgba(&out.join(format!("walls/{i:03}.png")), 64, 64, &img)?;
        walls.push(img);
    }
    out::sheet(&out.join("walls.png"), &walls, 64, 16)?;
    let mut sprites = Vec::new();
    for i in 0..pf.num_sprites() {
        let shape = pf.sprite(i).with_context(|| format!("sprite {i}"))?;
        let img = column_major_rgba(&shape.pixels, Some(&shape.opaque), &pal, transparent);
        out::png_rgba(&out.join(format!("sprites/{i:03}.png")), 64, 64, &img)?;
        sprites.push(img);
    }
    out::sheet(&out.join("sprites.png"), &sprites, 64, 20)?;
    let digi = pf.digi_sounds()?;
    for (i, s) in digi.iter().enumerate() {
        out::wav_u8(&out.join(format!("digi/{i:02}.wav")), 7042, &s.samples)?;
    }
    println!(
        "wrote {} walls, {} sprites, {} sounds to {}",
        walls.len(),
        sprites.len(),
        digi.len(),
        out.display()
    );
    Ok(())
}

fn column_major_rgba(px: &[u8], opaque: Option<&[bool]>, pal: &Palette, bg: [u8; 4]) -> Vec<u8> {
    let mut rgba = vec![0u8; SHAPE_SIZE * SHAPE_SIZE * 4];
    for x in 0..SHAPE_SIZE {
        for y in 0..SHAPE_SIZE {
            let src = x * SHAPE_SIZE + y;
            let c = if opaque.is_none_or(|o| o[src]) {
                let p = pal[px[src] as usize];
                [p[0], p[1], p[2], 255]
            } else {
                bg
            };
            rgba[(y * SHAPE_SIZE + x) * 4..][..4].copy_from_slice(&c);
        }
    }
    rgba
}

fn open_maps(data: &DataDir) -> Result<MapArchive> {
    Ok(MapArchive::parse(&data.read("MAPHEAD.BC")?, data.read("MAPTEMP.BC")?, Compression::Rlew)?)
}

fn maps(data: &DataDir, command: MapsCommand) -> Result<()> {
    let archive = open_maps(data)?;
    match command {
        MapsCommand::List => {
            for slot in archive.used_slots() {
                let m = archive.map(slot)?.context("map vanished")?;
                println!("{slot:3}  {:<16} {}x{}", m.name, m.width, m.height);
            }
        }
        MapsCommand::Stats { plane } => {
            let mut hist: BTreeMap<u16, (usize, Vec<usize>)> = BTreeMap::new();
            for slot in archive.used_slots() {
                let m = archive.map(slot)?.context("map vanished")?;
                for &v in &m.planes[plane] {
                    let e = hist.entry(v).or_default();
                    e.0 += 1;
                    if e.1.last() != Some(&slot) {
                        e.1.push(slot);
                    }
                }
            }
            for (v, (count, slots)) in hist {
                println!("{v:5} {count:7}  maps {}", compact(&slots));
            }
        }
        MapsCommand::Find { plane, value } => {
            for slot in archive.used_slots() {
                let m = archive.map(slot)?.context("map vanished")?;
                let hits: Vec<String> = (0..m.planes[plane].len())
                    .filter(|&i| m.planes[plane][i] == value)
                    .map(|i| format!("({},{})", i % m.width as usize, i / m.width as usize))
                    .collect();
                if !hits.is_empty() {
                    println!("{slot:3} {:<16} {}", m.name, hits.join(" "));
                }
            }
        }
        MapsCommand::Dump { slot, plane } => {
            let m = archive.map(slot)?.context("empty slot")?;
            println!("{} plane {plane}", m.name);
            for y in 0..m.height as usize {
                let row: Vec<String> =
                    (0..m.width as usize).map(|x| format!("{:3}", m.tile(plane, x, y))).collect();
                println!("{y:2}: {}", row.join(" "));
            }
        }
    }
    Ok(())
}

fn compact(slots: &[usize]) -> String {
    let mut parts = Vec::new();
    let mut i = 0;
    while i < slots.len() {
        let mut j = i;
        while j + 1 < slots.len() && slots[j + 1] == slots[j] + 1 {
            j += 1;
        }
        parts.push(if i == j {
            slots[i].to_string()
        } else {
            format!("{}-{}", slots[i], slots[j])
        });
        i = j + 1;
    }
    parts.join(",")
}

fn vga(data: &DataDir, out: &Path, palette_offset: Option<usize>) -> Result<()> {
    let pal = match palette_offset {
        Some(off) => MzExe::parse(data.read("COUNT.EXE")?)?.palette(off)?,
        None => palette(data)?,
    };
    let ga = GraphArchive::parse(
        &data.read("VGAHEAD.BC")?,
        &data.read("VGADICT.BC")?,
        data.read("VGAGRAPH.BC")?,
    )?;
    let table = ga.pic_table()?;
    println!("{} chunks, {} pics", ga.num_chunks(), table.len());
    for (i, &(w, h)) in table.iter().enumerate() {
        let chunk = ga.chunk(3 + i)?.with_context(|| format!("pic {i} sparse"))?;
        let pic = deplanarize(&chunk, w, h)?;
        let rgba: Vec<u8> = pic
            .pixels
            .iter()
            .flat_map(|&p| {
                let c = pal[p as usize];
                [c[0], c[1], c[2], 255]
            })
            .collect();
        out::png_rgba(&out.join(format!("pic{i:02}_{w}x{h}.png")), w as u32, h as u32, &rgba)?;
    }
    for chunk in [1, 2] {
        let Some(bytes) = ga.chunk(chunk)? else { continue };
        match Font::parse(&bytes) {
            Ok(font) => {
                let text: Vec<u8> = (32u8..127).collect();
                let w = font.measure(&text).max(1);
                let h = font.height as u32;
                let mut rgba = vec![0u8; (w * h * 4) as usize];
                let mut x0 = 0u32;
                for &c in &text {
                    let gw = font.width(c) as u32;
                    for (i, &v) in font.glyph(c).iter().enumerate() {
                        let (gx, gy) = (i as u32 % gw, i as u32 / gw);
                        if v != 0 {
                            let p = ((gy * w + x0 + gx) * 4) as usize;
                            rgba[p..p + 4].copy_from_slice(&[255, 255, 255, 255]);
                        }
                    }
                    x0 += gw;
                }
                out::png_rgba(&out.join(format!("font{chunk}.png")), w, h, &rgba)?;
                println!("chunk {chunk}: font, height {}", font.height);
            }
            Err(e) => println!("chunk {chunk}: not a font ({e})"),
        }
    }
    for chunk in table.len() + 3..ga.num_chunks() {
        let bytes = ga.chunk(chunk)?.unwrap_or_default();
        println!("chunk {chunk}: {} bytes {:02x?}", bytes.len(), &bytes[..bytes.len().min(16)]);
    }
    Ok(())
}

fn audio(data: &DataDir, out: &Path, audiomus_rate: u32, vswap_rate: u32) -> Result<()> {
    let archive = AudioArchive::parse(&data.read("AUDIOHED.BC")?, data.read("AUDIOT.BC")?)?;
    let n = archive.num_chunks();
    let mut pc = 0;
    for i in 0..n {
        if let Ok(s) = archive.pc_sound(i)
            && !s.data.is_empty()
            && i < 100
        {
            out::wav_u8(
                &out.join(format!("pc/{i:03}.wav")),
                44100,
                &out::pc_speaker(&s.data, 44100),
            )?;
            pc += 1;
        }
    }
    let sizes: Vec<usize> = (0..n).map(|i| archive.chunk(i).map_or(0, <[u8]>::len)).collect();
    println!("{n} AUDIOT chunks; wrote {pc} PC speaker sounds");
    println!("chunk sizes: {sizes:?}");
    let digi = parse_audiomus(&data.read("AUDIOMUS.BC")?)?;
    for (i, s) in digi.iter().enumerate() {
        out::wav_u8(&out.join(format!("audiomus/{i:02}.wav")), audiomus_rate, s)?;
    }
    println!("wrote {} AUDIOMUS sounds at {audiomus_rate} Hz", digi.len());
    let _ = vswap_rate;
    Ok(())
}

fn flc_export(data: &DataDir, out: &Path) -> Result<()> {
    let mut found = false;
    for name in ["F1.BC", "F2.BC", "F3.BC", "F4.BC", "F5.BC", "F6.BC"] {
        let Ok(bytes) = data.read(name) else { continue };
        found = true;
        let anim = flc::decode(&bytes).with_context(|| name.to_owned())?;
        println!(
            "{name}: {} frames {}x{} at {} ms",
            anim.frames.len(),
            anim.width,
            anim.height,
            anim.speed_ms
        );
        for (i, f) in anim.frames.iter().enumerate() {
            let rgba: Vec<u8> = f
                .pixels
                .iter()
                .flat_map(|&p| {
                    let c = f.palette[p as usize];
                    [c[0], c[1], c[2], 255]
                })
                .collect();
            let stem = name.trim_end_matches(".BC");
            out::png_rgba(
                &out.join(format!("{stem}/{i:02}.png")),
                anim.width as u32,
                anim.height as u32,
                &rgba,
            )?;
        }
    }
    if !found {
        bail!("no rendered sequences (F*.BC) in the data directory");
    }
    Ok(())
}
