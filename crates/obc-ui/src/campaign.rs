//! Campaigns: the ordered floors a game plays (docs/MODS_AND_OBCEDIT.md §4.2).
//!
//! The original game is the campaign of its 40 floors. A mod is a campaign
//! of its own floors, original floors by reference, or both, each with a
//! [`LevelProfile`] saying which original floor's behaviour it takes. `Game`
//! asks the campaign for floor `mapon`'s map and profile; finishing the last
//! floor wins the campaign.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use obc_formats::maps::{Map, MapArchive};
use obc_formats::modpack::{self, Mod};
use obc_sim::level::{LevelProfile, Severity, check_map};

use crate::assets::Assets;
use crate::screen::Ctx;

/// Where a floor's map comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapSource {
    /// An original floor, by map slot.
    Stock(u16),
    /// The mod's own map, by slot in its `MAPTEMP.BC`.
    Mod(u16),
}

#[derive(Debug, Clone)]
pub struct CampaignLevel {
    pub source: MapSource,
    pub profile: LevelProfile,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Campaign {
    /// The mod's id; empty for the original game.
    pub id: String,
    pub title: String,
    pub version: String,
    pub levels: Vec<CampaignLevel>,
    maps: Option<MapArchive>,
}

impl Campaign {
    /// The original game: floors 1-40.
    pub fn original(assets: &Assets) -> Self {
        let cd = assets.game.cd;
        let levels = (0..40u16)
            .map(|slot| CampaignLevel {
                source: MapSource::Stock(slot),
                profile: LevelProfile::stock(slot as i16, cd),
                name: assets.maps.name(usize::from(slot)).ok().flatten().unwrap_or_default(),
            })
            .collect();
        Self {
            id: String::new(),
            title: "OPERATION BODY COUNT".into(),
            version: String::new(),
            levels,
            maps: None,
        }
    }

    /// A mod's campaign; `cd` is the data release's.
    pub fn from_mod(m: &Mod, cd: bool) -> Self {
        let levels = m
            .manifest
            .levels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let source = match (l.map, l.stock) {
                    (Some(s), _) => MapSource::Mod(s),
                    (None, Some(s)) => MapSource::Stock(s),
                    (None, None) => MapSource::Stock(0), // refused by check_manifest
                };
                let name = l.name.clone().unwrap_or_else(|| {
                    let from = match source {
                        MapSource::Mod(s) => {
                            m.maps.as_ref().and_then(|a| a.name(usize::from(s)).ok().flatten())
                        }
                        MapSource::Stock(_) => None,
                    };
                    from.unwrap_or_else(|| format!("FLOOR {}", i + 1))
                });
                CampaignLevel { source, profile: LevelProfile::for_mod_level(l, cd), name }
            })
            .collect();
        Self {
            id: m.manifest.info.id.clone(),
            title: m.manifest.info.title.clone(),
            version: m.manifest.info.version.clone(),
            levels,
            maps: m.maps.clone(),
        }
    }

    pub fn is_original(&self) -> bool {
        self.id.is_empty()
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    pub fn level(&self, floor: i16) -> Option<&CampaignLevel> {
        usize::try_from(floor).ok().and_then(|i| self.levels.get(i))
    }

    /// Floor `floor`'s map.
    pub fn map(&self, assets: &Assets, floor: i16) -> Option<Map> {
        match self.level(floor)?.source {
            MapSource::Stock(s) => assets.maps.map(usize::from(s)).ok().flatten(),
            MapSource::Mod(s) => self.maps.as_ref()?.map(usize::from(s)).ok().flatten(),
        }
    }
}

/// Where installed mods live: `<user data>/mods`, beside the saves folder.
pub fn mods_dir(save_dir: &Path) -> PathBuf {
    save_dir.parent().unwrap_or(save_dir).join("mods")
}

/// Opens a mod and checks every map in it, as `--check-mod` and the MODS
/// screen do. Returns the mod and its problems (errors make it unplayable).
pub fn check(path: &Path) -> Result<(Mod, Vec<Problem>), String> {
    let m = modpack::open(path)?;
    let mut problems = Vec::new();
    for (i, l) in m.manifest.levels.iter().enumerate() {
        let Some(slot) = l.map else { continue };
        let Some(map) = m.maps.as_ref().and_then(|a| a.map(usize::from(slot)).ok().flatten())
        else {
            continue;
        };
        for p in check_map(&map) {
            problems.push(Problem {
                level: Some(i),
                at: p.at,
                error: p.severity == Severity::Error,
                message: p.message,
            });
        }
    }
    for name in &m.ignored {
        problems.push(Problem {
            level: None,
            at: None,
            error: false,
            message: format!("{name} is not used by OBCR"),
        });
    }
    Ok((m, problems))
}

/// One finding of [`check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub level: Option<usize>,
    pub at: Option<(u8, u8)>,
    pub error: bool,
    pub message: String,
}

/// A mod opened and checked, or why it could not be.
pub type Checked = Result<(Mod, Vec<Problem>), String>;

/// Every installed mod, opened and checked, for the MODS screen.
pub fn installed(save_dir: &Path) -> Vec<(PathBuf, Checked)> {
    let dir = mods_dir(save_dir);
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir() || p.extension().is_some_and(|x| x.eq_ignore_ascii_case(modpack::EXTENSION))
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let c = check(&p);
            (p, c)
        })
        .collect()
}

/// The mod `id` among the installed ones (manifests only, maps unchecked).
pub fn find_installed(save_dir: &Path, id: &str) -> Option<PathBuf> {
    modpack::scan(&mods_dir(save_dir))
        .into_iter()
        .find_map(|(p, r)| r.ok().filter(|m| m.id() == id).map(|_| p))
}

/// Whether a checked mod can be played (no errors, and its release fits).
pub fn playable(m: &Mod, problems: &[Problem], release: &str) -> Result<(), String> {
    if let Some(p) = problems.iter().find(|p| p.error) {
        return Err(match p.level {
            Some(l) => format!("floor {}: {}", l + 1, p.message),
            None => p.message.clone(),
        });
    }
    let want = m.manifest.info.release.as_str();
    if want != "any" && want != release {
        return Err(format!("needs the {want} release"));
    }
    Ok(())
}

/// The campaign a new game plays: the `--mod` path for this run, else the
/// mod chosen in the MODS screen, else the original. A mod that cannot be
/// played falls back to the original game, with the reason.
pub fn active(ctx: &Ctx) -> (Arc<Campaign>, Option<String>) {
    let g = &ctx.settings.game;
    let path = match (&g.mod_path, g.active_mod.as_str()) {
        (Some(p), _) => Some(p.clone()),
        (None, "") => None,
        (None, id) => match find_installed(ctx.save_dir, id) {
            Some(p) => Some(p),
            None => {
                let why = format!("mod {id} is not installed");
                return (Arc::new(Campaign::original(ctx.assets)), Some(why));
            }
        },
    };
    let Some(path) = path else { return (Arc::new(Campaign::original(ctx.assets)), None) };
    let release = ctx.assets.release.release.id;
    match check(&path).and_then(|(m, p)| playable(&m, &p, release).map(|()| m)) {
        Ok(m) => (Arc::new(Campaign::from_mod(&m, ctx.assets.game.cd)), None),
        Err(e) => {
            (Arc::new(Campaign::original(ctx.assets)), Some(format!("{}: {e}", path.display())))
        }
    }
}

/// The id and version of the campaign that is active (for scores and
/// saves), without checking its maps. Empty for the original game.
pub fn active_meta(ctx: &Ctx) -> (String, String) {
    let g = &ctx.settings.game;
    let path = match &g.mod_path {
        Some(p) => Some(p.clone()),
        None if g.active_mod.is_empty() => None,
        None => find_installed(ctx.save_dir, &g.active_mod),
    };
    match path.map(|p| modpack::open(&p)) {
        Some(Ok(m)) => (m.manifest.info.id.clone(), m.manifest.info.version.clone()),
        Some(Err(_)) | None => (g.active_mod.clone(), String::new()),
    }
}

/// The id of the active campaign; empty for the original game.
pub fn active_id(ctx: &Ctx) -> String {
    active_meta(ctx).0
}
