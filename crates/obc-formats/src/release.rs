//! Known releases of the original data and identification by SHA-256.

use sha2::{Digest, Sha256};

use crate::datadir::DataDir;

/// How a file matters to OBCR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Needed to run the game.
    Required,
    /// Rendered sequence (cutscene); optional.
    Sequence,
    /// Written by the original game (config, saves); contents vary per user.
    UserState,
    /// Shipped but not used by OBCR.
    Extra,
}

#[derive(Debug, Clone, Copy)]
pub struct KnownFile {
    pub name: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub role: Role,
}

#[derive(Debug)]
pub struct Release {
    pub id: &'static str,
    pub description: &'static str,
    pub files: &'static [KnownFile],
    /// File offset of `gamepal` in `COUNT.EXE`.
    pub gamepal_offset: usize,
}

const fn f(name: &'static str, size: u64, sha256: &'static str, role: Role) -> KnownFile {
    KnownFile { name, size, sha256, role }
}

/// The floppy release, files dated 1996-12-24 (docs/re/DATA_FORMATS.md §1).
pub static OBC_A: Release = Release {
    id: "OBC-A",
    description: "Operation Body Count, floppy release (files dated 1996-12-24)",
    files: &[
        f(
            "COUNT.EXE",
            235440,
            "67f3f7b1da7dedc6e0447e11943cb4f40f5453ee2c0b8dba3f10e11f4186c0c7",
            Role::Required,
        ),
        f(
            "GFXTILES.BC",
            2084536,
            "ee42cc2b6b8fe1f32ad203045cec01d82b268fe6784c115a3970d89dc47d9047",
            Role::Required,
        ),
        f(
            "MAPHEAD.BC",
            402,
            "fd49fcbe9cef41dc94d866ebde277a31271f73900a97f73b3257c9aaaf8d66e2",
            Role::Required,
        ),
        f(
            "MAPTEMP.BC",
            351762,
            "091a3a788a81687d4fff397b3ee147ef9b3f856b3ea981bb59d8a2d869a39269",
            Role::Required,
        ),
        f(
            "VGAHEAD.BC",
            177,
            "38b06e6dd2b68c4430fe46a2f4d62dd5b3530c6eab26c4127eceecf98601f562",
            Role::Required,
        ),
        f(
            "VGADICT.BC",
            1024,
            "52e9cdee7567ce77288aa9eef62696bd4e87fa4183e8c46ec6da587ede820c12",
            Role::Required,
        ),
        f(
            "VGAGRAPH.BC",
            317566,
            "1b607a1cf26e1ba41d859faebda002ee170797ff412483a4fe7be6472922c808",
            Role::Required,
        ),
        f(
            "AUDIOHED.BC",
            1244,
            "5dd52574b900516fd2307643cc124b694ac43e02c7057ac564de2c425220488b",
            Role::Required,
        ),
        f(
            "AUDIOT.BC",
            197419,
            "e11b82dc76e58abc67837a727929e14353386e49f049ade9c2cdd1636c190d59",
            Role::Required,
        ),
        f(
            "AUDIOMUS.BC",
            1376256,
            "182d0869101aa05ca12bee79491e3f0d318db6bda96c791d6eb2b82a6c58ca15",
            Role::Required,
        ),
        f(
            "F2.BC",
            843532,
            "09ab7a389f9937cde8f5b34a4db8bfd44ff62bea1d718e0b7947c4d06e8cedbd",
            Role::Sequence,
        ),
        f(
            "F3.BC",
            740692,
            "0de6ab6f78d775a87a1ce4d9fda8578d7d3610931403a916ea4809e969787cb3",
            Role::Sequence,
        ),
        f(
            "F4.BC",
            96492,
            "926cb093d0fa28655726b3dbfd2047d484460654a38d4a16be49025b0be61180",
            Role::Sequence,
        ),
        f(
            "F6.BC",
            207508,
            "c184bb63d42c5c42cb26cce1668c470fb2849c6c8df8e263ea0f3500c72a48ca",
            Role::Sequence,
        ),
        f("CONFIG.DAT", 287, "", Role::UserState),
        f(
            "SETUP.EXE",
            55134,
            "20e1c81dce781b7d42d75c408eb74ec2514b2335bb983d416bbd33396306fded",
            Role::Extra,
        ),
        f(
            "README.EXE",
            38702,
            "d531ebd593d73a351931d9fe3fa498371b4fb9bcaefdaa2f97b843db924e90d9",
            Role::Extra,
        ),
        f(
            "README.TXT",
            4585,
            "e7b5ceed345daa42d621abaae646d0be78016cb88a3a557ad03a8046ea6f3c06",
            Role::Extra,
        ),
        f(
            "BC.BAT",
            420,
            "e56f0a9f76d8f15e797561ac00e5aeb33a521c0f77401dfb37ca8b0c57bb8fb8",
            Role::Extra,
        ),
        f(
            "BCLOG.BAT",
            120,
            "2111caf6effc90b3aadcadc0ab09aa2186b0d489ad32a9cde01823c6e484f617",
            Role::Extra,
        ),
        f(
            "PRINTLOG.BAT",
            114,
            "2f7625c3f3914ba3fec6d48e54b064534b660b05b9ac75d842f446b953e1506a",
            Role::Extra,
        ),
    ],
    // Far pointer 3A9B:0000 passed to VL_FadeIn by the control panel
    // (ghidra 1618:061a); image 0x2A9B0.
    gamepal_offset: 0x2DDB0,
};

pub static RELEASES: &[&Release] = &[&OBC_A];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Ok,
    Missing,
    /// Present, hash not checked (user state).
    Present,
    Mismatch {
        size: u64,
        sha256: String,
    },
}

#[derive(Debug, Clone)]
pub struct FileCheck {
    pub file: KnownFile,
    pub status: FileStatus,
}

#[derive(Debug)]
pub struct Identification {
    pub release: &'static Release,
    pub checks: Vec<FileCheck>,
}

impl Identification {
    /// True if every required file matches.
    pub fn is_complete(&self) -> bool {
        self.checks.iter().all(|c| c.file.role != Role::Required || c.status == FileStatus::Ok)
    }

    pub fn has_sequences(&self) -> bool {
        self.checks.iter().any(|c| c.file.role == Role::Sequence && c.status == FileStatus::Ok)
    }
}

/// Lower-case hex SHA-256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// Checks `dir` against every known release and returns the best match
/// (the one with the most matching required files).
pub fn identify(dir: &DataDir) -> Identification {
    let mut best: Option<(usize, Identification)> = None;
    for release in RELEASES {
        let checks: Vec<FileCheck> = release
            .files
            .iter()
            .map(|file| FileCheck { file: *file, status: check(dir, file) })
            .collect();
        let score = checks.iter().filter(|c| c.status == FileStatus::Ok).count();
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, Identification { release, checks }));
        }
    }
    best.map(|(_, id)| id).unwrap_or_else(|| Identification { release: &OBC_A, checks: Vec::new() })
}

fn check(dir: &DataDir, file: &KnownFile) -> FileStatus {
    let Ok(data) = dir.read(file.name) else {
        return FileStatus::Missing;
    };
    if file.role == Role::UserState {
        return FileStatus::Present;
    }
    let sha256 = sha256_hex(&data);
    if data.len() as u64 == file.size && sha256 == file.sha256 {
        FileStatus::Ok
    } else {
        FileStatus::Mismatch { size: data.len() as u64, sha256 }
    }
}
