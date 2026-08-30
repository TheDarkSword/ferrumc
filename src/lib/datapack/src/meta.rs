//! `pack.mcmeta`: what version of the game a pack was written for, and how it describes itself.

use ferrumc_text::TextComponent;
use serde_json::Value;
use std::fmt;

include!(concat!(env!("OUT_DIR"), "/pack_format.rs"));

/// The last pack format before packs gained a minor version. A pack that claims anything above it
/// has to say so with `min_format`/`max_format`; the older `pack_format` field cannot express it.
///
/// Vanilla's `PackFormat.lastPreMinorVersion(SERVER_DATA)`.
pub const LAST_PRE_MINOR: u32 = 81;

/// A pack format version, `major.minor`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PackFormat {
    pub major: u32,
    pub minor: u32,
}

impl PackFormat {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// The version a bare major number means, which is its first minor release.
    pub const fn of(major: u32) -> Self {
        Self::new(major, 0)
    }

    /// This major version and every minor release of it, which is what a pack built for the
    /// running game supports.
    pub const fn minor_range(self) -> (Self, Self) {
        (self, Self::new(self.major, u32::MAX))
    }
}

impl fmt::Display for PackFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.minor == u32::MAX {
            write!(f, "{}.*", self.major)
        } else {
            write!(f, "{}.{}", self.major, self.minor)
        }
    }
}

/// Whether a pack was written for a game this old or this new.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackCompatibility {
    TooOld,
    TooNew,
    /// The pack says nothing about which version it is for.
    Unknown,
    Compatible,
}

impl PackCompatibility {
    /// Vanilla's `PackCompatibility.forVersion`.
    pub fn of(declared: (PackFormat, PackFormat), game: PackFormat) -> Self {
        let (min, max) = declared;
        if min.major == u32::MAX {
            Self::Unknown
        } else if max < game {
            Self::TooOld
        } else if game < min {
            Self::TooNew
        } else {
            Self::Compatible
        }
    }

    pub fn is_compatible(self) -> bool {
        self == Self::Compatible
    }
}

/// What a pack says about itself.
#[derive(Clone, Debug)]
pub struct PackMetadata {
    pub description: TextComponent,
    /// The lowest and highest pack format the pack claims to work with.
    pub supported_formats: (PackFormat, PackFormat),
}

/// Why a `pack.mcmeta` could not be read.
#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("pack.mcmeta is not valid json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("pack.mcmeta has no \"pack\" section")]
    NoPackSection,
    #[error("pack.mcmeta has no description")]
    NoDescription,
    #[error("pack.mcmeta is missing a field, must declare both min_format and max_format")]
    HalfDeclared,
    #[error("pack.mcmeta min_format ({0}) is greater than max_format ({1})")]
    Inverted(PackFormat, PackFormat),
    #[error(
        "pack.mcmeta declares support for a version newer than {LAST_PRE_MINOR}, but is missing \
         min_format and max_format"
    )]
    NeedsMinAndMax,
    #[error("pack.mcmeta carries no format version information")]
    NoFormat,
}

impl PackMetadata {
    /// Reads a `pack.mcmeta`.
    pub fn parse(bytes: &[u8]) -> Result<Self, MetadataError> {
        let root: Value = serde_json::from_slice(bytes)?;
        let pack = root.get("pack").ok_or(MetadataError::NoPackSection)?;
        let description = pack
            .get("description")
            .ok_or(MetadataError::NoDescription)
            .map(ferrumc_text::from_json)?;
        Ok(Self {
            description,
            supported_formats: supported_formats(pack)?,
        })
    }
}

/// Works out which pack formats a `pack` section claims, in vanilla's order of preference:
/// the `min_format`/`max_format` pair, then the deprecated `supported_formats` range, then the
/// single `pack_format`.
fn supported_formats(pack: &Value) -> Result<(PackFormat, PackFormat), MetadataError> {
    // An absent minor means the bottom of the major version for `min_format`, and the top of it
    // for `max_format`, so that a pair of bare majors covers everything between them.
    let min = pack.get("min_format").and_then(|v| read_format(v, 0));
    let max = pack
        .get("max_format")
        .and_then(|v| read_format(v, u32::MAX));

    match (min, max) {
        (Some(min), Some(max)) if min > max => Err(MetadataError::Inverted(min, max)),
        (Some(min), Some(max)) => Ok((min, max)),
        (Some(_), None) | (None, Some(_)) => Err(MetadataError::HalfDeclared),
        (None, None) => {
            if let Some((min, max)) = pack.get("supported_formats").and_then(read_range) {
                if max > LAST_PRE_MINOR {
                    return Err(MetadataError::NeedsMinAndMax);
                }
                return Ok((PackFormat::of(min), PackFormat::of(max)));
            }
            match pack.get("pack_format").and_then(Value::as_u64) {
                Some(format) => {
                    let format = u32::try_from(format).unwrap_or(u32::MAX);
                    if format > LAST_PRE_MINOR {
                        Err(MetadataError::NeedsMinAndMax)
                    } else {
                        let format = PackFormat::of(format);
                        Ok((format, format))
                    }
                }
                None => Err(MetadataError::NoFormat),
            }
        }
    }
}

/// A format is either a bare major version or a `[major, minor]` pair.
fn read_format(value: &Value, default_minor: u32) -> Option<PackFormat> {
    if let Some(major) = value.as_u64() {
        return Some(PackFormat::new(u32::try_from(major).ok()?, default_minor));
    }
    let pair = value.as_array()?;
    let major = u32::try_from(pair.first()?.as_u64()?).ok()?;
    match pair.get(1) {
        Some(minor) => Some(PackFormat::new(major, u32::try_from(minor.as_u64()?).ok()?)),
        None => Some(PackFormat::new(major, default_minor)),
    }
}

/// A range is either `[min, max]` or `{"min_inclusive": .., "max_inclusive": ..}`.
fn read_range(value: &Value) -> Option<(u32, u32)> {
    if let Some(pair) = value.as_array() {
        let min = u32::try_from(pair.first()?.as_u64()?).ok()?;
        let max = u32::try_from(pair.get(1)?.as_u64()?).ok()?;
        return Some((min, max));
    }
    let min = u32::try_from(value.get("min_inclusive")?.as_u64()?).ok()?;
    let max = u32::try_from(value.get("max_inclusive")?.as_u64()?).ok()?;
    Some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(json: &str) -> Result<PackMetadata, MetadataError> {
        PackMetadata::parse(json.as_bytes())
    }

    #[test]
    fn reads_a_min_max_pair() {
        let parsed =
            meta(r#"{"pack":{"description":"hi","min_format":[107,0],"max_format":[107,5]}}"#)
                .expect("a readable pack.mcmeta");
        assert_eq!(
            parsed.supported_formats,
            (PackFormat::new(107, 0), PackFormat::new(107, 5))
        );
    }

    #[test]
    fn a_bare_major_means_the_whole_major_version() {
        let parsed = meta(r#"{"pack":{"description":"hi","min_format":107,"max_format":107}}"#)
            .expect("a readable pack.mcmeta");
        assert_eq!(
            parsed.supported_formats,
            (PackFormat::new(107, 0), PackFormat::new(107, u32::MAX))
        );
    }

    #[test]
    fn the_old_single_field_only_reaches_the_versions_it_can_name() {
        let parsed = meta(r#"{"pack":{"description":"hi","pack_format":81}}"#)
            .expect("a readable pack.mcmeta");
        assert_eq!(
            parsed.supported_formats,
            (PackFormat::of(81), PackFormat::of(81))
        );
        assert!(matches!(
            meta(r#"{"pack":{"description":"hi","pack_format":107}}"#),
            Err(MetadataError::NeedsMinAndMax)
        ));
    }

    #[test]
    fn half_a_declaration_is_refused() {
        assert!(matches!(
            meta(r#"{"pack":{"description":"hi","min_format":[107,0]}}"#),
            Err(MetadataError::HalfDeclared)
        ));
        assert!(matches!(
            meta(r#"{"pack":{"description":"hi"}}"#),
            Err(MetadataError::NoFormat)
        ));
    }

    #[test]
    fn compares_against_the_running_game() {
        let game = PackFormat::new(107, 1);
        assert_eq!(
            PackCompatibility::of(PackFormat::new(107, 1).minor_range(), game),
            PackCompatibility::Compatible
        );
        assert_eq!(
            PackCompatibility::of((PackFormat::of(80), PackFormat::of(81)), game),
            PackCompatibility::TooOld
        );
        assert_eq!(
            PackCompatibility::of(PackFormat::new(108, 0).minor_range(), game),
            PackCompatibility::TooNew
        );
        // A pack built for 107.0 does not cover 107.1: a minor release can add to the format.
        assert_eq!(
            PackCompatibility::of((PackFormat::of(107), PackFormat::of(107)), game),
            PackCompatibility::TooOld
        );
    }

    #[test]
    fn a_description_reads_in_all_three_shapes() {
        assert_eq!(
            meta(r#"{"pack":{"description":"plain","pack_format":81}}"#)
                .expect("a readable pack.mcmeta")
                .description,
            TextComponent::from("plain")
        );
        let joined = meta(r#"{"pack":{"description":["a",{"text":"b"}],"pack_format":81}}"#)
            .expect("a readable pack.mcmeta")
            .description;
        assert_eq!(joined.extra.len(), 1);
        let object = meta(r#"{"pack":{"description":{"text":"o"},"pack_format":81}}"#)
            .expect("a readable pack.mcmeta")
            .description;
        assert_eq!(object, TextComponent::from("o"));
    }
}
