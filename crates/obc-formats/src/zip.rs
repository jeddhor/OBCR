//! A small, bounded ZIP reader for `.obcmod` files (docs/MODS_AND_OBCEDIT.md
//! §3.1, §4.3).
//!
//! A mod is untrusted input, so this reads only what a mod needs and refuses
//! everything else before decompressing anything: stored and deflated entries
//! only, no encryption, no ZIP64, no multi-disk archives, no names that leave
//! the archive's root (absolute paths, drive letters, `..`, backslashes), and
//! hard limits on entry count and sizes. CRC-32 is checked on every read.

use std::collections::BTreeMap;

use crate::error::{FormatError, Result};

/// Most entries an archive may hold.
pub const MAX_ENTRIES: usize = 2000;
/// Largest uncompressed entry.
pub const MAX_ENTRY: u64 = 16 << 20;
/// Largest total uncompressed size.
pub const MAX_TOTAL: u64 = 128 << 20;

const EOCD: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

fn invalid(msg: impl Into<String>) -> FormatError {
    FormatError::Invalid { what: "ZIP archive", msg: msg.into() }
}

fn u16_at(b: &[u8], at: usize) -> Result<u16> {
    b.get(at..at + 2).map(|s| u16::from_le_bytes([s[0], s[1]])).ok_or_else(|| invalid("truncated"))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        .ok_or_else(|| invalid("truncated"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    method: u16,
    crc: u32,
    compressed: u64,
    size: u64,
    local: u64,
}

/// An opened archive: its bytes and a directory of safe file names.
#[derive(Debug, Clone)]
pub struct ZipArchive {
    data: Vec<u8>,
    /// Upper-case-insensitive lookup: upper-case name → (original name, entry).
    entries: BTreeMap<String, (String, Entry)>,
}

/// Whether `name` stays inside the archive's root.
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains('\\')
        && !name.contains(':')
        && !name.chars().any(char::is_control)
        && name.split('/').all(|part| part != ".." && part != ".")
}

impl ZipArchive {
    pub fn parse(data: Vec<u8>) -> Result<Self> {
        // The end-of-central-directory record is 22 bytes plus a comment of
        // up to 65535 bytes, at the very end.
        let min = data.len().saturating_sub(22 + 0xFFFF);
        let eocd = (min..=data.len().saturating_sub(22))
            .rev()
            .find(|&i| u32_at(&data, i).ok() == Some(EOCD))
            .ok_or_else(|| invalid("no end of central directory: not a ZIP file"))?;
        let disk = u16_at(&data, eocd + 4)?;
        let cd_disk = u16_at(&data, eocd + 6)?;
        let count = u16_at(&data, eocd + 10)?;
        let cd_size = u32_at(&data, eocd + 12)?;
        let cd_off = u32_at(&data, eocd + 16)?;
        if disk != 0 || cd_disk != 0 {
            return Err(invalid("multi-disk archives are not supported"));
        }
        if count == 0xFFFF || cd_size == u32::MAX || cd_off == u32::MAX {
            return Err(invalid("ZIP64 archives are not supported"));
        }
        if usize::from(count) > MAX_ENTRIES {
            return Err(invalid(format!("{count} entries (at most {MAX_ENTRIES})")));
        }
        let mut entries = BTreeMap::new();
        let mut at = cd_off as usize;
        let mut total = 0u64;
        for _ in 0..count {
            if u32_at(&data, at)? != CENTRAL {
                return Err(invalid("bad central directory record"));
            }
            let flags = u16_at(&data, at + 8)?;
            let method = u16_at(&data, at + 10)?;
            let crc = u32_at(&data, at + 16)?;
            let compressed = u64::from(u32_at(&data, at + 20)?);
            let size = u64::from(u32_at(&data, at + 24)?);
            let name_len = usize::from(u16_at(&data, at + 28)?);
            let extra_len = usize::from(u16_at(&data, at + 30)?);
            let comment_len = usize::from(u16_at(&data, at + 32)?);
            let local = u64::from(u32_at(&data, at + 42)?);
            let raw =
                data.get(at + 46..at + 46 + name_len).ok_or_else(|| invalid("truncated name"))?;
            let name =
                String::from_utf8(raw.to_vec()).map_err(|_| invalid("a file name is not UTF-8"))?;
            at += 46 + name_len + extra_len + comment_len;
            if name.ends_with('/') {
                continue; // a directory
            }
            if !safe_name(&name) {
                return Err(invalid(format!("unsafe file name {name:?}")));
            }
            if flags & 1 != 0 {
                return Err(invalid(format!("{name} is encrypted")));
            }
            if method != 0 && method != 8 {
                return Err(invalid(format!(
                    "{name}: compression method {method} (only stored and deflate)"
                )));
            }
            if size > MAX_ENTRY {
                return Err(invalid(format!("{name} is {size} bytes (at most {MAX_ENTRY})")));
            }
            total += size;
            if total > MAX_TOTAL {
                return Err(invalid(format!("more than {MAX_TOTAL} bytes in all")));
            }
            let key = name.to_ascii_uppercase();
            if entries.contains_key(&key) {
                return Err(invalid(format!("{name} appears twice")));
            }
            entries.insert(key, (name, Entry { method, crc, compressed, size, local }));
        }
        Ok(Self { data, entries })
    }

    /// File names, as stored.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.values().map(|(n, _)| n.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&name.to_ascii_uppercase())
    }

    /// Reads `name` (any case), decompressed and CRC-checked.
    pub fn read(&self, name: &str) -> Result<Vec<u8>> {
        let (_, e) = self
            .entries
            .get(&name.to_ascii_uppercase())
            .ok_or_else(|| FormatError::Missing(name.to_owned()))?;
        let at = e.local as usize;
        if u32_at(&self.data, at)? != LOCAL {
            return Err(invalid(format!("{name}: bad local header")));
        }
        let skip = 30
            + usize::from(u16_at(&self.data, at + 26)?)
            + usize::from(u16_at(&self.data, at + 28)?);
        let start = at + skip;
        let raw = self
            .data
            .get(start..start + e.compressed as usize)
            .ok_or_else(|| invalid(format!("{name} is truncated")))?;
        let out = if e.method == 0 {
            raw.to_vec()
        } else {
            miniz_oxide::inflate::decompress_to_vec_with_limit(raw, e.size as usize)
                .map_err(|_| invalid(format!("{name}: bad deflate data")))?
        };
        if out.len() as u64 != e.size {
            return Err(invalid(format!(
                "{name}: {} bytes, the directory says {}",
                out.len(),
                e.size
            )));
        }
        if crc32(&out) != e.crc {
            return Err(invalid(format!("{name}: CRC mismatch")));
        }
        Ok(out)
    }
}

/// CRC-32 (IEEE 802.3), as ZIP uses.
pub fn crc32(data: &[u8]) -> u32 {
    const TABLE: [u32; 256] = {
        let mut t = [0u32; 256];
        let mut i = 0;
        while i < 256 {
            let mut c = i as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
                k += 1;
            }
            t[i] = c;
            i += 1;
        }
        t
    };
    !data.iter().fold(!0u32, |c, &b| TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8))
}

/// Writes a ZIP of stored entries (tests, and tools that need no compression).
pub fn write_stored(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, body) in files {
        let local = out.len() as u32;
        let crc = crc32(body);
        let head = |sig: u32, out: &mut Vec<u8>| {
            out.extend_from_slice(&sig.to_le_bytes());
            if sig == CENTRAL {
                out.extend_from_slice(&20u16.to_le_bytes()); // made by
            }
            out.extend_from_slice(&20u16.to_le_bytes()); // needed
            out.extend_from_slice(&0u16.to_le_bytes()); // flags
            out.extend_from_slice(&0u16.to_le_bytes()); // stored
            out.extend_from_slice(&[0; 4]); // time, date
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // extra
        };
        head(LOCAL, &mut out);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
        head(CENTRAL, &mut central);
        central.extend_from_slice(&[0; 6]); // comment len, disk, internal attrs
        central.extend_from_slice(&[0; 4]); // external attrs
        central.extend_from_slice(&local.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_off = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&EOCD.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_of_known_text() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn reads_what_it_writes() {
        let z =
            ZipArchive::parse(write_stored(&[("mod.toml", b"x = 1"), ("dir/Readme.txt", b"hi")]))
                .unwrap();
        assert_eq!(z.read("MOD.TOML").unwrap(), b"x = 1");
        assert_eq!(z.read("dir/readme.txt").unwrap(), b"hi");
        assert!(z.read("missing").is_err());
    }

    #[test]
    fn reads_deflate() {
        let body = b"hello hello hello hello hello".repeat(20);
        let packed = miniz_oxide::deflate::compress_to_vec(&body, 6);
        let mut z = write_stored(&[("a.txt", &packed)]);
        // Turn the entry into a deflated one: method 8, real size and CRC.
        let crc = crc32(&body).to_le_bytes();
        let size = (body.len() as u32).to_le_bytes();
        z[8] = 8;
        z[14..18].copy_from_slice(&crc);
        z[22..26].copy_from_slice(&size);
        let cd = z.windows(4).position(|w| w == CENTRAL.to_le_bytes()).unwrap();
        z[cd + 10] = 8;
        z[cd + 16..cd + 20].copy_from_slice(&crc);
        z[cd + 24..cd + 28].copy_from_slice(&size);
        assert_eq!(ZipArchive::parse(z).unwrap().read("a.txt").unwrap(), body);
    }

    #[test]
    fn refuses_unsafe_names_and_damage() {
        for bad in ["../evil", "/etc/passwd", "C:/x", "a\\b", "x/../../y"] {
            assert!(ZipArchive::parse(write_stored(&[(bad, b"")])).is_err(), "{bad}");
        }
        let mut z = write_stored(&[("a.txt", b"data")]);
        z[30 + 5] ^= 1; // corrupt the body
        assert!(ZipArchive::parse(z).unwrap().read("a.txt").is_err(), "CRC");
        assert!(ZipArchive::parse(b"not a zip".to_vec()).is_err());
        assert!(ZipArchive::parse(write_stored(&[("a", b""), ("A", b"")])).is_err(), "duplicate");
    }

    proptest::proptest! {
        #[test]
        fn never_panics(data in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512)) {
            if let Ok(z) = ZipArchive::parse(data) {
                let names: Vec<String> = z.names().map(str::to_owned).collect();
                for n in names {
                    let _ = z.read(&n);
                }
            }
        }
    }
}
