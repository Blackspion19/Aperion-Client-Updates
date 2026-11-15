//! Resource pack format handling (`pack.mcmeta`).
//!
//! Minecraft identifies the layout of a resource pack with a "pack format"
//! number. Two things changed recently and are handled here:
//!
//! * Since Minecraft 1.21.9 (25w31a) `pack.mcmeta` uses `min_format` /
//!   `max_format` instead of `pack_format` / `supported_formats`. The old
//!   fields must be absent for packs that only target 1.21.9 or newer.
//! * Since 2026 Minecraft uses year based versions (`26.1`, `26.2`, ...).
//!
//! See <https://minecraft.wiki/w/Pack.mcmeta> and
//! <https://minecraft.wiki/w/Pack_format>.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// First resource pack format (Minecraft 1.21.9) that is declared through
/// `min_format` / `max_format` rather than `pack_format`.
pub const MIN_MAX_FORMAT_SINCE: u32 = 65;

/// Upper bound written to `max_format`. Pack formats only ever increase, so
/// an open upper bound keeps a locally created pack usable on newer game
/// versions without Minecraft showing a "made for an older version" warning.
pub const OPEN_MAX_FORMAT: u32 = 999;

/// The resource pack format a pack targets.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackFormat(pub u32);

/// `(first game version, resource pack format)`, sorted ascending.
///
/// Game versions are `(major, minor, patch)`: `1.21.11` is `(1, 21, 11)` and
/// `26.2` is `(26, 2, 0)`. Sources: Minecraft Wiki "Pack format" and
/// published pack releases for each version.
const FORMAT_TABLE: &[((u32, u32, u32), u32)] = &[
    ((1, 6, 0), 1),
    ((1, 9, 0), 2),
    ((1, 11, 0), 3),
    ((1, 13, 0), 4),
    ((1, 15, 0), 5),
    ((1, 16, 2), 6),
    ((1, 17, 0), 7),
    ((1, 18, 0), 8),
    ((1, 19, 0), 9),
    ((1, 19, 3), 12),
    ((1, 19, 4), 13),
    ((1, 20, 0), 15),
    ((1, 20, 2), 18),
    ((1, 20, 3), 22),
    ((1, 20, 5), 32),
    ((1, 21, 0), 34),
    ((1, 21, 2), 42),
    ((1, 21, 4), 46),
    ((1, 21, 5), 55),
    ((1, 21, 6), 63),
    ((1, 21, 7), 64),
    ((1, 21, 9), 69),
    ((1, 21, 11), 75),
    ((26, 1, 0), 84),
    ((26, 2, 0), 88),
];

/// Parses `1.21.11`, `26.2`, `1.21.4-pre1`, `26.3-snapshot-6`, ... into
/// `(major, minor, patch)`. Returns `None` for anything that is not a
/// `major.minor[.patch]` release (for example old-style snapshots like
/// `25w31a`).
fn parse_game_version(game_version: &str) -> Option<(u32, u32, u32)> {
    let core = game_version
        .trim()
        .split(|c: char| c == '-' || c == ' ' || c == '_')
        .next()?;
    let mut parts = core.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = match parts.next() {
        Some(part) => part.parse().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

impl PackFormat {
    /// The newest format known to this launcher.
    pub fn latest_known() -> Self {
        Self(FORMAT_TABLE[FORMAT_TABLE.len() - 1].1)
    }

    /// Resolve the resource pack format for a game version string.
    ///
    /// Versions newer than the table (for example a `26.3` that this
    /// launcher does not know yet) get the newest known format. Together
    /// with the open `max_format` this still produces a pack that Minecraft
    /// accepts, because formats only grow. Unparseable versions (snapshots)
    /// also fall back to the newest known format.
    pub fn for_game_version(game_version: &str) -> Self {
        let Some(version) = parse_game_version(game_version) else {
            return Self::latest_known();
        };

        let mut format = FORMAT_TABLE[0].1;
        for (start, candidate) in FORMAT_TABLE {
            if version >= *start {
                format = *candidate;
            }
        }
        Self(format)
    }

    /// Whether `pack.mcmeta` has to use `min_format` / `max_format`.
    pub fn uses_min_max_format(self) -> bool {
        self.0 >= MIN_MAX_FORMAT_SINCE
    }

    /// The `pack.mcmeta` content for a pack that works from `min` up to
    /// `max`. `max = None` means no upper limit ("and newer").
    ///
    /// One file serves every Minecraft generation:
    /// * `pack_format` for versions before 1.20.2,
    /// * `supported_formats` for 1.20.2 to 1.21.8,
    /// * `min_format` / `max_format` from 1.21.9 on.
    pub fn mcmeta_range(min: Self, max: Option<Self>, description: &str) -> Value {
        let lowest = min.0;
        let highest_known = max.map(|format| format.0.max(lowest));
        let mut pack = serde_json::Map::new();
        pack.insert("description".to_string(), json!(description));

        if lowest < MIN_MAX_FORMAT_SINCE {
            pack.insert("pack_format".to_string(), json!(lowest));
            // `supported_formats` exists since format 18 (1.20.2)
            let upper = highest_known.unwrap_or(Self::latest_known().0);
            if lowest >= 18 && upper > lowest {
                pack.insert(
                    "supported_formats".to_string(),
                    json!({ "min_inclusive": lowest, "max_inclusive": upper }),
                );
            }
        }

        let upper_new = highest_known.unwrap_or(OPEN_MAX_FORMAT);
        if upper_new >= MIN_MAX_FORMAT_SINCE {
            pack.insert("min_format".to_string(), json!(lowest));
            pack.insert("max_format".to_string(), json!(upper_new));
        }

        json!({ "pack": Value::Object(pack) })
    }

    /// The `pack.mcmeta` content for a pack with this format (single version).
    pub fn mcmeta(self, description: &str) -> Value {
        if self.uses_min_max_format() {
            json!({
                "pack": {
                    "description": description,
                    "min_format": self.0,
                    "max_format": OPEN_MAX_FORMAT,
                }
            })
        } else {
            json!({
                "pack": {
                    "pack_format": self.0,
                    "description": description,
                }
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(version: &str) -> u32 {
        PackFormat::for_game_version(version).0
    }

    #[test]
    fn modern_versions() {
        assert_eq!(format("1.21.11"), 75);
        assert_eq!(format("1.21.10"), 69);
        assert_eq!(format("1.21.9"), 69);
        assert_eq!(format("1.21.8"), 64);
        assert_eq!(format("1.21.7"), 64);
        assert_eq!(format("1.21.6"), 63);
        assert_eq!(format("1.21.5"), 55);
        assert_eq!(format("1.21.4"), 46);
        assert_eq!(format("1.21.3"), 42);
        assert_eq!(format("1.21.2"), 42);
        assert_eq!(format("1.21.1"), 34);
        assert_eq!(format("1.21"), 34);
    }

    #[test]
    fn year_based_versions() {
        assert_eq!(format("26.1"), 84);
        assert_eq!(format("26.1.2"), 84);
        assert_eq!(format("26.2"), 88);
        // Newer than the table: newest known format.
        assert_eq!(format("26.3"), 88);
        assert_eq!(format("27.1"), 88);
    }

    #[test]
    fn older_versions() {
        assert_eq!(format("1.20.6"), 32);
        assert_eq!(format("1.20.4"), 22);
        assert_eq!(format("1.20.2"), 18);
        assert_eq!(format("1.20.1"), 15);
        assert_eq!(format("1.19.4"), 13);
        assert_eq!(format("1.19.3"), 12);
        assert_eq!(format("1.19.2"), 9);
        assert_eq!(format("1.18.2"), 8);
        assert_eq!(format("1.16.5"), 6);
        assert_eq!(format("1.16.1"), 5);
        assert_eq!(format("1.12.2"), 3);
        assert_eq!(format("1.8.9"), 1);
    }

    #[test]
    fn prereleases_and_unknown() {
        assert_eq!(format("1.21.4-pre1"), 46);
        assert_eq!(format("26.3-snapshot-6"), 88);
        assert_eq!(format("25w31a"), 88);
        assert_eq!(format(""), 88);
    }

    #[test]
    fn mcmeta_shape() {
        let old = PackFormat(34).mcmeta("old");
        assert_eq!(old["pack"]["pack_format"], 34);
        assert!(old["pack"].get("min_format").is_none());

        let new = PackFormat(75).mcmeta("new");
        assert!(new["pack"].get("pack_format").is_none());
        assert!(new["pack"].get("supported_formats").is_none());
        assert_eq!(new["pack"]["min_format"], 75);
        assert_eq!(new["pack"]["max_format"], OPEN_MAX_FORMAT);
        assert_eq!(new["pack"]["description"], "new");

        assert!(!PackFormat(64).uses_min_max_format());
        assert!(PackFormat(65).uses_min_max_format());
    }
}
