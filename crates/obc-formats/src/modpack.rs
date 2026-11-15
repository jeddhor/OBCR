//! Mods: a folder or a `.obcmod` ZIP with a `mod.toml` manifest and the mod's
//! maps in the original format (docs/MODS_AND_OBCEDIT.md §3).
//!
//! This module reads and checks the package: the manifest's schema and
//! limits, and that every referenced map exists, decodes and is 64×64. What a
//! map contains (player starts, doors, codes) is checked by the simulation,
//! which knows the engine's limits (`obc_sim::level::check_map`).

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::maps::{Compression, MapArchive, NUM_MAP_SLOTS};
use crate::zip::ZipArchive;

/// The newest mod format this version reads.
pub const FORMAT: u32 = 1;
/// Most levels a campaign may have.
pub const MAX_LEVELS: usize = 100;
/// The file extension of a packed mod.
pub const EXTENSION: &str = "obcmod";

/// Everything wrong with a mod, as a sentence (shown in the MODS screen).
pub type ModError = String;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    #[serde(rename = "mod")]
    pub info: ModInfo,
    #[serde(rename = "level", default)]
    pub levels: Vec<LevelEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModInfo {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// `any`, `OBC-A` or `OBC-CD`.
    #[serde(default = "any")]
    pub release: String,
    /// The oldest OBCR that plays it (informational).
    #[serde(default)]
    pub obcr: String,
}

fn any() -> String {
    "any".into()
}

/// A song number or `"random"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum MusicSpec {
    Song(u16),
    Named(MusicWord),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MusicWord {
    Random,
}

/// One floor of the campaign (§3.2's table).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LevelEntry {
    pub map: Option<u16>,
    pub stock: Option<u16>,
    pub name: Option<String>,
    pub like: Option<u16>,
    pub music: Option<MusicSpec>,
    pub dark: Option<bool>,
    pub sewer: Option<bool>,
    pub colors: Option<u16>,
    pub sequence: Option<String>,
    pub message: Option<String>,
    pub exit: Option<String>,
}

/// An opened, checked mod.
#[derive(Debug, Clone)]
pub struct Mod {
    pub manifest: Manifest,
    /// The mod's own maps, if any level uses one.
    pub maps: Option<MapArchive>,
    pub readme: Option<String>,
    /// Where it was loaded from.
    pub path: PathBuf,
    /// Files in the package that OBCR does not read (reported, not refused).
    pub ignored: Vec<String>,
}

impl Mod {
    pub fn id(&self) -> &str {
        &self.manifest.info.id
    }
}

/// Files in a mod package, from a folder or a ZIP.
enum Source {
    Dir(PathBuf),
    Zip(Box<ZipArchive>),
}

impl Source {
    fn read(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            Self::Dir(d) => {
                let entries = std::fs::read_dir(d).ok()?;
                for e in entries.flatten() {
                    if e.file_name().to_string_lossy().eq_ignore_ascii_case(name)
                        && e.file_type().ok()?.is_file()
                    {
                        let meta = e.metadata().ok()?;
                        if meta.len() > crate::zip::MAX_ENTRY {
                            return None;
                        }
                        return std::fs::read(e.path()).ok();
                    }
                }
                None
            }
            Self::Zip(z) => z.read(name).ok(),
        }
    }

    fn names(&self) -> Vec<String> {
        match self {
            Self::Dir(d) => std::fs::read_dir(d)
                .map(|it| {
                    it.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()
                })
                .unwrap_or_default(),
            Self::Zip(z) => z.names().map(str::to_owned).collect(),
        }
    }
}

fn printable(s: &str, max: usize) -> bool {
    s.len() <= max && s.bytes().all(|b| (0x20..0x7F).contains(&b))
}

fn valid_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Checks the manifest's fields (not the maps).
pub fn check_manifest(m: &Manifest) -> Result<(), ModError> {
    if m.format == 0 || m.format > FORMAT {
        return Err(format!("mod format {} (this OBCR reads up to {FORMAT})", m.format));
    }
    let i = &m.info;
    if !valid_id(&i.id) {
        return Err(format!("id {:?}: use 1-32 characters a-z, 0-9 and '-'", i.id));
    }
    for (what, s, max) in [
        ("title", &i.title, 32),
        ("author", &i.author, 64),
        ("version", &i.version, 16),
        ("description", &i.description, 400),
        ("obcr", &i.obcr, 32),
    ] {
        if !printable(s, max) {
            return Err(format!("{what}: at most {max} printable ASCII characters"));
        }
    }
    if i.title.is_empty() {
        return Err("title is empty".into());
    }
    if !matches!(i.release.as_str(), "any" | "OBC-A" | "OBC-CD") {
        return Err(format!("release {:?}: any, OBC-A or OBC-CD", i.release));
    }
    if m.levels.is_empty() {
        return Err("no [[level]]: a campaign needs at least one floor".into());
    }
    if m.levels.len() > MAX_LEVELS {
        return Err(format!("{} levels (at most {MAX_LEVELS})", m.levels.len()));
    }
    for (n, l) in m.levels.iter().enumerate() {
        let at = |msg: String| format!("level {}: {msg}", n + 1);
        match (l.map, l.stock) {
            (Some(_), Some(_)) | (None, None) => {
                return Err(at("give exactly one of map and stock".into()));
            }
            (Some(s), None) if usize::from(s) >= NUM_MAP_SLOTS => {
                return Err(at(format!("map {s} (slots 0-{})", NUM_MAP_SLOTS - 1)));
            }
            (None, Some(s)) if s > 39 => {
                return Err(at(format!("stock {s} (original floors 0-39)")));
            }
            _ => {}
        }
        if let Some(s) = l.like
            && s > 39
        {
            return Err(at(format!("like {s} (original floors 0-39)")));
        }
        if let Some(c) = l.colors
            && c > 50
        {
            return Err(at(format!("colors {c} (0-50)")));
        }
        if let Some(MusicSpec::Song(s)) = l.music
            && s > 9
        {
            return Err(at(format!("music {s} (songs 0-9, or \"random\")")));
        }
        if let Some(seq) = &l.sequence
            && !matches!(
                seq.as_str(),
                "auto" | "none" | "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7"
            )
        {
            return Err(at(format!("sequence {seq:?}: auto, none or F1-F7")));
        }
        if let Some(e) = &l.exit
            && !matches!(e.as_str(), "normal" | "none")
        {
            return Err(at(format!("exit {e:?}: normal or none")));
        }
        if let Some(name) = &l.name
            && !printable(name, 16)
        {
            return Err(at("name: at most 16 printable ASCII characters".into()));
        }
        if let Some(msg) = &l.message
            && !printable(msg, 40)
        {
            return Err(at("message: at most 40 printable ASCII characters".into()));
        }
    }
    Ok(())
}

/// Opens a mod folder or `.obcmod` file and checks its package and maps' shape.
pub fn open(path: &Path) -> Result<Mod, ModError> {
    let src = if path.is_dir() {
        Source::Dir(path.to_owned())
    } else {
        let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.len() > crate::zip::MAX_TOTAL {
            return Err(format!(
                "{} is larger than {} bytes",
                path.display(),
                crate::zip::MAX_TOTAL
            ));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Source::Zip(Box::new(ZipArchive::parse(bytes).map_err(|e| e.to_string())?))
    };
    let text = src.read("mod.toml").ok_or("no mod.toml")?;
    let text = String::from_utf8(text).map_err(|_| "mod.toml is not UTF-8".to_owned())?;
    let manifest: Manifest =
        toml::from_str(&text).map_err(|e| format!("mod.toml: {}", e.message()))?;
    check_manifest(&manifest)?;
    let needs_maps = manifest.levels.iter().any(|l| l.map.is_some());
    let maps = if needs_maps {
        let head = src.read("MAPHEAD.BC").ok_or("no MAPHEAD.BC (a level uses map =)")?;
        let body = src.read("MAPTEMP.BC").ok_or("no MAPTEMP.BC (a level uses map =)")?;
        Some(MapArchive::parse(&head, body, Compression::Rlew).map_err(|e| e.to_string())?)
    } else {
        None
    };
    for (n, l) in manifest.levels.iter().enumerate() {
        let (Some(slot), Some(maps)) = (l.map, &maps) else { continue };
        let map = maps
            .map(usize::from(slot))
            .map_err(|e| format!("level {}: map {slot}: {e}", n + 1))?
            .ok_or_else(|| format!("level {}: map {slot} is not in MAPTEMP.BC", n + 1))?;
        if map.width != 64 || map.height != 64 {
            return Err(format!(
                "level {}: map {slot} is {}x{} (Operation Body Count maps are 64x64)",
                n + 1,
                map.width,
                map.height
            ));
        }
    }
    let readme = src.read("README.txt").map(|b| String::from_utf8_lossy(&b).into_owned());
    let known = ["MOD.TOML", "MAPHEAD.BC", "MAPTEMP.BC", "README.TXT"];
    let ignored = src
        .names()
        .into_iter()
        .filter(|n| !known.contains(&n.to_ascii_uppercase().as_str()))
        .collect();
    Ok(Mod { manifest, maps, readme, path: path.to_owned(), ignored })
}

/// Every mod in `dir` (folders and `.obcmod` files), sorted by file name,
/// each opened or with the reason it cannot be.
pub fn scan(dir: &Path) -> Vec<(PathBuf, Result<Mod, ModError>)> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() || p.extension().is_some_and(|x| x.eq_ignore_ascii_case(EXTENSION)))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let m = open(&p);
            (p, m)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(levels: &str) -> String {
        format!("format = 1\n[mod]\nid = \"t\"\ntitle = \"T\"\n{levels}")
    }

    fn parse(text: &str) -> Result<Manifest, ModError> {
        let m: Manifest = toml::from_str(text).map_err(|e| e.message().to_owned())?;
        check_manifest(&m)?;
        Ok(m)
    }

    #[test]
    fn reads_the_documented_manifest() {
        let m = parse(&manifest(
            "[[level]]\nmap = 0\nname = \"Lobby\"\nlike = 6\n[[level]]\nstock = 7\n[[level]]\nmap = 1\nlike = 2\nmusic = 4\n[[level]]\nmap = 2\nmusic = \"random\"\nsequence = \"none\"\nexit = \"none\"\n",
        ))
        .unwrap();
        assert_eq!(m.levels.len(), 4);
        assert_eq!(m.levels[2].music, Some(MusicSpec::Song(4)));
        assert_eq!(m.levels[3].music, Some(MusicSpec::Named(MusicWord::Random)));
        assert_eq!(m.info.release, "any");
    }

    #[test]
    fn refuses_bad_manifests() {
        for (bad, why) in [
            (manifest(""), "no levels"),
            (manifest("[[level]]\nmap = 0\nstock = 1\n"), "both"),
            (manifest("[[level]]\n"), "neither"),
            (manifest("[[level]]\nstock = 40\n"), "stock range"),
            (manifest("[[level]]\nmap = 0\nmusic = 12\n"), "music range"),
            (manifest("[[level]]\nmap = 0\nsequence = \"F9\"\n"), "sequence"),
            (manifest("[[level]]\nmap = 0\nteleport = true\n"), "unknown key"),
            (manifest("[[level]]\nmap = 0\nname = \"\u{1}\"\n"), "control char"),
            ("format = 2\n[mod]\nid = \"t\"\ntitle = \"T\"\n[[level]]\nmap = 0\n".into(), "format"),
            (
                "format = 1\n[mod]\nid = \"Bad Id\"\ntitle = \"T\"\n[[level]]\nmap = 0\n".into(),
                "id",
            ),
        ] {
            assert!(parse(&bad).is_err(), "{why}");
        }
    }

    #[test]
    fn opens_a_stock_only_zip_and_lists_extras() {
        let dir = std::env::temp_dir().join(format!("obcr-modpack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("stock.obcmod");
        let toml = manifest("[[level]]\nstock = 3\n");
        std::fs::write(
            &path,
            crate::zip::write_stored(&[("mod.toml", toml.as_bytes()), ("notes.md", b"x")]),
        )
        .unwrap();
        let m = open(&path).unwrap();
        assert_eq!(
            (m.id(), m.maps.is_none(), m.ignored.clone()),
            ("t", true, vec!["notes.md".to_owned()])
        );
        let path2 = dir.join("broken.obcmod");
        std::fs::write(
            &path2,
            crate::zip::write_stored(&[("mod.toml", manifest("[[level]]\nmap = 0\n").as_bytes())]),
        )
        .unwrap();
        assert!(open(&path2).unwrap_err().contains("MAPHEAD"));
        let found = scan(&dir);
        assert_eq!(found.len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
