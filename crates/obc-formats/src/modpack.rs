//! Mods: a folder or a `.obcmod` ZIP with a `mod.toml` manifest and the mod's
//! maps in the original format (docs/MODS_AND_OBCEDIT.md §3), and from
//! format 2 its own art, sounds, music and network arenas
//! (docs/MOD_ASSETS_AND_ARENAS.md §2).
//!
//! This module reads and checks the package: the manifest's schema and
//! limits, that every referenced map exists, decodes and is 64×64, and that
//! every asset decodes. What a map contains (player starts, doors, codes) is
//! checked by the simulation, which knows the engine's limits
//! (`obc_sim::level::check_map`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::maps::{Compression, MapArchive, NUM_MAP_SLOTS};
use crate::modassets::{self, AssetIndex};
use crate::zip::ZipArchive;

/// The newest mod format this version reads.
pub const FORMAT: u32 = 2;
/// Most levels a campaign may have.
pub const MAX_LEVELS: usize = 100;
/// Most network arenas a mod may have (the original's ten slots, 50-59).
pub const MAX_ARENAS: usize = 10;
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
    #[serde(rename = "arena", default)]
    pub arenas: Vec<ArenaEntry>,
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

/// A song number, `"random"`, or (format 2) a music file of the mod.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum MusicSpec {
    Song(u16),
    Named(MusicWord),
    File(String),
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

/// A network arena (format 2; docs/MOD_ASSETS_AND_ARENAS.md §2.2).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ArenaEntry {
    pub map: u16,
    pub name: Option<String>,
    pub music: Option<MusicSpec>,
    pub colors: Option<u16>,
    pub dark: Option<bool>,
    pub sewer: Option<bool>,
}

/// A mod's files, kept open for reading its assets when a game starts.
#[derive(Debug)]
pub struct Package {
    src: Source,
    /// Its asset files (empty below format 2).
    pub assets: AssetIndex,
    /// Every file's name and size, for limits, the check cache and the
    /// content hash.
    files: Vec<(String, u64, Option<std::time::SystemTime>)>,
}

impl Package {
    /// A file of the package by its path (`walls/012.png`), case-insensitive.
    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.src.read(path)
    }

    /// A music file named in the manifest.
    pub fn music(&self, name: &str) -> Option<Vec<u8>> {
        self.read(self.assets.music_path(name)?)
    }
}

/// An opened, checked mod.
#[derive(Debug, Clone)]
pub struct Mod {
    pub manifest: Manifest,
    /// The mod's own maps, if any level or arena uses one.
    pub maps: Option<MapArchive>,
    pub readme: Option<String>,
    /// Where it was loaded from.
    pub path: PathBuf,
    /// Files in the package that OBCR does not read (reported, not refused).
    pub ignored: Vec<String>,
    pub package: Arc<Package>,
}

impl Mod {
    pub fn id(&self) -> &str {
        &self.manifest.info.id
    }

    /// SHA-256 over every file OBCR reads from the mod, in name order, with
    /// the names: two mods are the same mod for a network game when this
    /// matches (docs/MOD_ASSETS_AND_ARENAS.md §3.5).
    pub fn content_hash(&self) -> [u8; 32] {
        let mut names: Vec<&str> = self
            .package
            .files
            .iter()
            .map(|(n, _, _)| n.as_str())
            .filter(|n| !self.ignored.iter().any(|i| i == n))
            .collect();
        names.sort_by_key(|n| n.to_ascii_lowercase());
        let mut h = Sha256::new();
        for n in names {
            let bytes = self.package.read(n).unwrap_or_default();
            h.update(n.to_ascii_lowercase().as_bytes());
            h.update([0]);
            h.update((bytes.len() as u64).to_le_bytes());
            h.update(&bytes);
        }
        h.finalize().into()
    }
}

/// Files in a mod package, from a folder or a ZIP.
#[derive(Debug)]
enum Source {
    Dir(PathBuf),
    Zip(Box<ZipArchive>),
}

/// The entry of `dir` named `name`, case-insensitively.
fn find_in(dir: &Path, name: &str) -> Option<std::fs::DirEntry> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
}

impl Source {
    fn read(&self, name: &str) -> Option<Vec<u8>> {
        match self {
            Self::Dir(d) => {
                let mut at = d.clone();
                let parts: Vec<&str> = name.split('/').collect();
                for (i, part) in parts.iter().enumerate() {
                    if part.is_empty() || *part == "." || *part == ".." {
                        return None;
                    }
                    let e = find_in(&at, part)?;
                    let ft = e.file_type().ok()?;
                    let last = i + 1 == parts.len();
                    // Never follow links out of the mod folder.
                    if ft.is_symlink() || (last && !ft.is_file()) || (!last && !ft.is_dir()) {
                        return None;
                    }
                    at = e.path();
                }
                if std::fs::metadata(&at).ok()?.len() > crate::zip::MAX_ENTRY {
                    return None;
                }
                std::fs::read(at).ok()
            }
            Self::Zip(z) => z.read(name).ok(),
        }
    }

    /// Every file, with its size and (for folders) modification time. A
    /// folder's subfolders are listed one level deep (`walls/012.png`).
    fn files(&self) -> Vec<(String, u64, Option<std::time::SystemTime>)> {
        match self {
            Self::Dir(d) => {
                let mut out = Vec::new();
                let Ok(top) = std::fs::read_dir(d) else { return out };
                for e in top.flatten() {
                    let Ok(ft) = e.file_type() else { continue };
                    let name = e.file_name().to_string_lossy().into_owned();
                    if ft.is_file() {
                        let m = e.metadata().ok();
                        out.push((
                            name,
                            m.as_ref().map_or(0, |m| m.len()),
                            m.and_then(|m| m.modified().ok()),
                        ));
                    } else if ft.is_dir() {
                        let Ok(sub) = std::fs::read_dir(e.path()) else { continue };
                        for s in sub.flatten() {
                            let Ok(st) = s.file_type() else { continue };
                            let sname = format!("{name}/{}", s.file_name().to_string_lossy());
                            if st.is_file() {
                                let m = s.metadata().ok();
                                out.push((
                                    sname,
                                    m.as_ref().map_or(0, |m| m.len()),
                                    m.and_then(|m| m.modified().ok()),
                                ));
                            } else {
                                out.push((sname + "/", 0, None));
                            }
                        }
                    }
                }
                out.sort();
                out
            }
            Self::Zip(z) => {
                z.names().filter(|n| !n.ends_with('/')).map(|n| (n.to_owned(), 0, None)).collect()
            }
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

fn check_music_spec(format: u32, m: &Option<MusicSpec>) -> Result<(), String> {
    match m {
        Some(MusicSpec::Song(s)) if *s > 9 => {
            Err(format!("music {s} (songs 0-9, \"random\", or a music file)"))
        }
        Some(MusicSpec::File(name)) => {
            if format < 2 {
                return Err(format!("music {name:?}: music files need format = 2"));
            }
            modassets::music_name(name).map(|_| ())
        }
        _ => Ok(()),
    }
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
        check_music_spec(m.format, &l.music).map_err(at)?;
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
    if !m.arenas.is_empty() && m.format < 2 {
        return Err("[[arena]] needs format = 2".into());
    }
    if m.arenas.len() > MAX_ARENAS {
        return Err(format!("{} arenas (at most {MAX_ARENAS})", m.arenas.len()));
    }
    for (n, a) in m.arenas.iter().enumerate() {
        let at = |msg: String| format!("arena {}: {msg}", n + 1);
        if usize::from(a.map) >= NUM_MAP_SLOTS {
            return Err(at(format!("map {} (slots 0-{})", a.map, NUM_MAP_SLOTS - 1)));
        }
        if let Some(c) = a.colors
            && c > 59
        {
            return Err(at(format!("colors {c} (0-59)")));
        }
        check_music_spec(m.format, &a.music).map_err(at)?;
        if let Some(name) = &a.name
            && !printable(name, 16)
        {
            return Err(at("name: at most 16 printable ASCII characters".into()));
        }
    }
    Ok(())
}

/// The music files a manifest names (levels and arenas).
fn named_music(m: &Manifest) -> impl Iterator<Item = &str> {
    m.levels.iter().map(|l| &l.music).chain(m.arenas.iter().map(|a| &a.music)).filter_map(|s| {
        match s {
            Some(MusicSpec::File(f)) => Some(f.as_str()),
            _ => None,
        }
    })
}

/// Opens a mod folder or `.obcmod` file and checks its package, its maps'
/// shape and its asset files' names (their contents: [`check_assets`]).
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
    let files = src.files();
    // A folder gets the ZIP reader's limits too.
    if files.len() > crate::zip::MAX_ENTRIES {
        return Err(format!("{} files (at most {})", files.len(), crate::zip::MAX_ENTRIES));
    }
    let total: u64 = files.iter().map(|f| f.1).sum();
    if total > crate::zip::MAX_TOTAL {
        return Err(format!("more than {} bytes in all", crate::zip::MAX_TOTAL));
    }
    if let Some((n, size, _)) = files.iter().find(|f| f.1 > crate::zip::MAX_ENTRY) {
        return Err(format!("{n} is {size} bytes (at most {})", crate::zip::MAX_ENTRY));
    }
    let text = src.read("mod.toml").ok_or("no mod.toml")?;
    let text = String::from_utf8(text).map_err(|_| "mod.toml is not UTF-8".to_owned())?;
    let manifest: Manifest =
        toml::from_str(&text).map_err(|e| format!("mod.toml: {}", e.message()))?;
    check_manifest(&manifest)?;
    let needs_maps = manifest.levels.iter().any(|l| l.map.is_some()) || !manifest.arenas.is_empty();
    let maps = if needs_maps {
        let head = src.read("MAPHEAD.BC").ok_or("no MAPHEAD.BC (a level or arena uses map =)")?;
        let body = src.read("MAPTEMP.BC").ok_or("no MAPTEMP.BC (a level or arena uses map =)")?;
        Some(MapArchive::parse(&head, body, Compression::Rlew).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let slots = manifest
        .levels
        .iter()
        .enumerate()
        .filter_map(|(n, l)| l.map.map(|s| (format!("level {}", n + 1), s)))
        .chain(
            manifest.arenas.iter().enumerate().map(|(n, a)| (format!("arena {}", n + 1), a.map)),
        );
    for (what, slot) in slots {
        let Some(maps) = &maps else { continue };
        let map = maps
            .map(usize::from(slot))
            .map_err(|e| format!("{what}: map {slot}: {e}"))?
            .ok_or_else(|| format!("{what}: map {slot} is not in MAPTEMP.BC"))?;
        if map.width != 64 || map.height != 64 {
            return Err(format!(
                "{what}: map {slot} is {}x{} (Operation Body Count maps are 64x64)",
                map.width, map.height
            ));
        }
    }
    let readme = src.read("README.txt").map(|b| String::from_utf8_lossy(&b).into_owned());
    let known = ["MOD.TOML", "MAPHEAD.BC", "MAPTEMP.BC", "README.TXT"];
    let names: Vec<&str> = files.iter().map(|f| f.0.as_str()).collect();
    let assets = if manifest.format >= 2 {
        AssetIndex::build(names.iter().copied())?
    } else {
        AssetIndex::default()
    };
    for f in named_music(&manifest) {
        if assets.music_path(f).is_none() {
            return Err(format!("music {f:?} is not in the mod's music folder"));
        }
    }
    let ignored = names
        .iter()
        .filter(|n| !known.contains(&n.to_ascii_uppercase().as_str()))
        .filter(|n| manifest.format < 2 || modassets::classify(n).is_none())
        .map(|n| (*n).to_owned())
        .collect();
    let package = Arc::new(Package { src, assets, files });
    Ok(Mod { manifest, maps, readme, path: path.to_owned(), ignored, package })
}

/// One finding of [`check_assets`]: `true` for an error.
pub type AssetProblem = (bool, String);

type CheckKey = (PathBuf, Vec<(String, u64, Option<std::time::SystemTime>)>);

/// Verdicts of [`check_assets`] by mod path and its files' sizes and times,
/// so the MODS screen and each game start do not decode a mod's music again.
fn cache() -> &'static Mutex<HashMap<CheckKey, Vec<AssetProblem>>> {
    static CACHE: std::sync::OnceLock<Mutex<HashMap<CheckKey, Vec<AssetProblem>>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Decodes every asset of a mod in full: a file that would fail in the game
/// fails here. Music nothing names is a warning. A ZIP's verdict is cached
/// by path only when the file's size and time are unchanged (folders by
/// every file's size and time).
pub fn check_assets(m: &Mod) -> Vec<AssetProblem> {
    let stamp = match &m.package.src {
        Source::Dir(_) => m.package.files.clone(),
        Source::Zip(_) => std::fs::metadata(&m.path)
            .map(|md| vec![(String::new(), md.len(), md.modified().ok())])
            .unwrap_or_default(),
    };
    let key = (m.path.clone(), stamp);
    if let Some(hit) = cache().lock().ok().and_then(|c| c.get(&key).cloned()) {
        return hit;
    }
    let out = check_assets_uncached(m);
    if let Ok(mut c) = cache().lock() {
        if c.len() > 64 {
            c.clear();
        }
        c.insert(key, out.clone());
    }
    out
}

fn check_assets_uncached(m: &Mod) -> Vec<AssetProblem> {
    let a = &m.package.assets;
    let mut out = Vec::new();
    let mut err = |path: &str, msg: String| out.push((true, format!("{path}: {msg}")));
    for path in a.walls.values().chain(a.sprites.values()) {
        match m.package.read(path) {
            Some(b) => {
                if let Err(e) = modassets::decode_png(&b) {
                    err(path, e);
                }
            }
            None => err(path, "cannot be read".into()),
        }
    }
    for path in a.sounds.values() {
        match m.package.read(path) {
            Some(b) => {
                if let Err(e) = modassets::decode_wav(&b) {
                    err(path, e);
                }
            }
            None => err(path, "cannot be read".into()),
        }
    }
    for path in a.music.values() {
        let file = path.split_once('/').map_or(path.as_str(), |(_, f)| f);
        match m.package.read(path) {
            Some(b) => {
                if let Err(e) = modassets::check_music(file, Arc::from(b)) {
                    err(path, e);
                }
            }
            None => err(path, "cannot be read".into()),
        }
    }
    let used: Vec<String> = named_music(&m.manifest).map(str::to_ascii_lowercase).collect();
    for (name, path) in &a.music {
        if !used.contains(name) {
            out.push((false, format!("{path} is not used by any level or arena")));
        }
    }
    out
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

    fn manifest2(rest: &str) -> String {
        format!("format = 2\n[mod]\nid = \"t\"\ntitle = \"T\"\n{rest}")
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
        assert!(m.arenas.is_empty());
    }

    #[test]
    fn reads_format_2() {
        let m = parse(&manifest2(
            "[[level]]\nmap = 0\nmusic = \"theme.ogg\"\n[[arena]]\nmap = 1\nname = \"Pit\"\nmusic = \"random\"\ncolors = 55\n[[arena]]\nmap = 2\n",
        ))
        .unwrap();
        assert_eq!(m.levels[0].music, Some(MusicSpec::File("theme.ogg".into())));
        assert_eq!(m.arenas.len(), 2);
        assert_eq!(m.arenas[0].colors, Some(55));
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
            (manifest("[[level]]\nmap = 0\nmusic = \"a.ogg\"\n"), "music file in format 1"),
            (manifest("[[level]]\nmap = 0\n[[arena]]\nmap = 1\n"), "arena in format 1"),
            (manifest2("[[level]]\nmap = 0\nmusic = \"a.mp3\"\n"), "music extension"),
            (manifest2("[[level]]\nmap = 0\nmusic = \"a b.ogg\"\n"), "music name"),
            (manifest2("[[level]]\nmap = 0\n[[arena]]\nmap = 1\ncolors = 60\n"), "arena colors"),
            (manifest2("[[level]]\nmap = 0\n[[arena]]\nstock = 1\n"), "arena stock"),
            (
                manifest2(
                    &("[[level]]\nmap = 0\n".to_owned() + &"[[arena]]\nmap = 1\n".repeat(11)),
                ),
                "11 arenas",
            ),
            ("format = 3\n[mod]\nid = \"t\"\ntitle = \"T\"\n[[level]]\nmap = 0\n".into(), "format"),
            (
                "format = 1\n[mod]\nid = \"Bad Id\"\ntitle = \"T\"\n[[level]]\nmap = 0\n".into(),
                "id",
            ),
        ] {
            assert!(parse(&bad).is_err(), "{why}");
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("obcr-modpack-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn opens_a_stock_only_zip_and_lists_extras() {
        let dir = tmp("zip");
        let path = dir.join("stock.obcmod");
        let toml = manifest("[[level]]\nstock = 3\n");
        std::fs::write(
            &path,
            crate::zip::write_stored(&[
                ("mod.toml", toml.as_bytes()),
                ("notes.md", b"x"),
                ("walls/001.png", b"x"),
            ]),
        )
        .unwrap();
        let m = open(&path).unwrap();
        assert_eq!(
            (m.id(), m.maps.is_none(), m.ignored.clone()),
            ("t", true, vec!["notes.md".to_owned(), "walls/001.png".to_owned()]),
            "a format-1 mod does not use asset folders"
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

    fn tiny_png(color: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, 64, 64);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().unwrap();
        w.write_image_data(&color.repeat(64 * 64)).unwrap();
        w.finish().unwrap();
        out
    }

    #[test]
    fn opens_and_checks_a_format_2_folder() {
        let dir = tmp("dir2");
        std::fs::create_dir_all(dir.join("Walls")).unwrap();
        std::fs::create_dir_all(dir.join("music")).unwrap();
        std::fs::create_dir_all(dir.join("sounds")).unwrap();
        std::fs::write(
            dir.join("mod.toml"),
            manifest2("[[level]]\nstock = 3\nmusic = \"Tone.ogg\"\n"),
        )
        .unwrap();
        std::fs::write(dir.join("Walls/012.png"), tiny_png([1, 2, 3, 255])).unwrap();
        std::fs::write(dir.join("music/tone.ogg"), include_bytes!("../tests/data/tone.ogg"))
            .unwrap();
        std::fs::write(dir.join("music/spare.ogg"), include_bytes!("../tests/data/tone.ogg"))
            .unwrap();
        std::fs::write(dir.join("sounds/03.wav"), b"RIFF nonsense").unwrap();
        let m = open(&dir).unwrap();
        assert_eq!(m.package.assets.walls[&12], "Walls/012.png");
        assert!(m.package.read("walls/012.png").is_some(), "case-insensitive");
        assert!(m.package.music("TONE.OGG").is_some());
        assert!(m.ignored.is_empty(), "{:?}", m.ignored);
        let p = check_assets(&m);
        assert!(p.iter().any(|(e, msg)| *e && msg.contains("sounds/03.wav")), "{p:?}");
        assert!(p.iter().any(|(e, msg)| !*e && msg.contains("spare.ogg")), "{p:?}");
        assert_eq!(p.len(), 2, "{p:?}");
        // The hash covers every byte OBCR reads.
        let h = m.content_hash();
        std::fs::write(dir.join("Walls/012.png"), tiny_png([1, 2, 4, 255])).unwrap();
        assert_ne!(open(&dir).unwrap().content_hash(), h);
        // A named music file must exist.
        std::fs::write(
            dir.join("mod.toml"),
            manifest2("[[level]]\nstock = 3\nmusic = \"gone.ogg\"\n"),
        )
        .unwrap();
        assert!(open(&dir).unwrap_err().contains("gone.ogg"));
        // A badly named asset is an error, not ignored.
        std::fs::write(dir.join("mod.toml"), manifest2("[[level]]\nstock = 3\n")).unwrap();
        std::fs::write(dir.join("Walls/999.png"), b"x").unwrap();
        assert!(open(&dir).unwrap_err().contains("999"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_folder_never_reads_outside_itself() {
        let dir = tmp("escape");
        std::fs::write(dir.join("mod.toml"), manifest("[[level]]\nstock = 3\n")).unwrap();
        let m = open(&dir).unwrap();
        for bad in ["../x", "./mod.toml", "/mod.toml", "music/../mod.toml"] {
            assert!(m.package.read(bad).is_none(), "{bad}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
