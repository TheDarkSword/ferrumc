//! Resource locations: `namespace:path`, the key everything in a datapack is addressed by.

use std::fmt;
use std::path::{Path, PathBuf};

/// The namespace a location gets when it does not say one.
pub const DEFAULT_NAMESPACE: &str = "minecraft";

/// A namespaced identifier.
///
/// Held as the single string it prints as, plus where the separator sits, so a location costs one
/// allocation and hashes over the bytes it was parsed from.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    full: Box<str>,
    sep: u16,
}

/// Why a string is not a resource location.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IdentifierError {
    #[error("non [a-z0-9_.-] character in namespace of identifier: {0}")]
    Namespace(String),
    #[error("non [a-z0-9/._-] character in path of identifier: {0}")]
    Path(String),
    #[error("namespace of identifier is too long: {0}")]
    NamespaceTooLong(String),
}

impl Identifier {
    /// Builds a location from its two halves.
    pub fn new(namespace: &str, path: &str) -> Result<Self, IdentifierError> {
        if !valid_namespace(namespace) {
            return Err(IdentifierError::Namespace(format!("{namespace}:{path}")));
        }
        if !valid_path(path) {
            return Err(IdentifierError::Path(format!("{namespace}:{path}")));
        }
        let sep = u16::try_from(namespace.len())
            .map_err(|_| IdentifierError::NamespaceTooLong(namespace.to_string()))?;
        Ok(Self {
            full: format!("{namespace}:{path}").into_boxed_str(),
            sep,
        })
    }

    /// A location in the default namespace.
    pub fn minecraft(path: &str) -> Result<Self, IdentifierError> {
        Self::new(DEFAULT_NAMESPACE, path)
    }

    /// Reads `namespace:path`, or `path` alone in the default namespace.
    pub fn parse(s: &str) -> Result<Self, IdentifierError> {
        match s.split_once(':') {
            Some(("", path)) => Self::minecraft(path),
            Some((namespace, path)) => Self::new(namespace, path),
            None => Self::minecraft(s),
        }
    }

    pub fn namespace(&self) -> &str {
        &self.full[..usize::from(self.sep)]
    }

    pub fn path(&self) -> &str {
        &self.full[usize::from(self.sep) + 1..]
    }

    /// The whole thing, `namespace:path`.
    pub fn as_str(&self) -> &str {
        &self.full
    }

    /// The same namespace with a different path.
    pub fn with_path(&self, path: &str) -> Result<Self, IdentifierError> {
        Self::new(self.namespace(), path)
    }

    /// Where this location's file sits under `top`, which is a pack's `data/` directory.
    ///
    /// Returns `None` when the path holds a segment that could climb out of the pack, which the
    /// character rules alone allow: `.` and `/` are both legal in a path.
    pub fn resolve_against(&self, top: &Path) -> Option<PathBuf> {
        let mut resolved = top.join(self.namespace());
        for segment in decompose_path(self.path())? {
            resolved.push(segment);
        }
        Some(resolved)
    }
}

/// Splits a resource path into the segments it addresses, refusing any that could leave the pack.
///
/// Vanilla's `FileUtil.decomposePath`: no empty segment, no `.` or `..`, and every segment made of
/// `[-._a-z0-9]`.
pub fn decompose_path(path: &str) -> Option<Vec<&str>> {
    let segments: Vec<&str> = path.split('/').collect();
    for segment in &segments {
        if segment.is_empty() || *segment == "." || *segment == ".." {
            return None;
        }
        if !segment
            .bytes()
            .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
        {
            return None;
        }
    }
    Some(segments)
}

fn valid_namespace(namespace: &str) -> bool {
    namespace != ".."
        && namespace
            .bytes()
            .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
}

fn valid_path(path: &str) -> bool {
    path.bytes()
        .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/'))
}

impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full)
    }
}

impl fmt::Debug for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.full)
    }
}

/// Ordered by path first, as vanilla does, so a listing groups the same file across namespaces.
impl Ord for Identifier {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.path()
            .cmp(other.path())
            .then_with(|| self.namespace().cmp(other.namespace()))
    }
}

impl PartialOrd for Identifier {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_the_separator() {
        let id = Identifier::parse("mypack:tags/block/logs").expect("a valid location");
        assert_eq!(id.namespace(), "mypack");
        assert_eq!(id.path(), "tags/block/logs");
        assert_eq!(id.as_str(), "mypack:tags/block/logs");
    }

    #[test]
    fn a_bare_path_is_minecraft() {
        let id = Identifier::parse("stone").expect("a valid location");
        assert_eq!(id.namespace(), "minecraft");
        assert_eq!(id.to_string(), "minecraft:stone");
        // A leading colon means the same thing.
        assert_eq!(Identifier::parse(":stone").expect("a valid location"), id);
    }

    #[test]
    fn rejects_characters_outside_the_allowed_set() {
        assert!(Identifier::parse("Stone").is_err());
        assert!(Identifier::parse("my pack:stone").is_err());
        assert!(Identifier::new("..", "stone").is_err());
        // A slash belongs to the path and not to the namespace.
        assert!(Identifier::new("a/b", "stone").is_err());
    }

    #[test]
    fn a_path_cannot_climb_out_of_the_pack() {
        let root = Path::new("/packs/vanilla/data");
        // Legal characters, and still an escape: this is what the segment rules are for.
        let escaping = Identifier::new("minecraft", "../../etc/passwd").expect("a valid location");
        assert_eq!(escaping.resolve_against(root), None);
        assert_eq!(decompose_path("a//b"), None);
        assert_eq!(decompose_path("a/./b"), None);

        let ordinary = Identifier::minecraft("tags/block/logs").expect("a valid location");
        assert_eq!(
            ordinary.resolve_against(root),
            Some(PathBuf::from(
                "/packs/vanilla/data/minecraft/tags/block/logs"
            ))
        );
    }

    #[test]
    fn ordered_by_path_before_namespace() {
        let mut ids = [
            Identifier::parse("zed:a").expect("a valid location"),
            Identifier::parse("alpha:b").expect("a valid location"),
            Identifier::parse("alpha:a").expect("a valid location"),
        ];
        ids.sort();
        assert_eq!(
            ids.map(|id| id.to_string()),
            ["alpha:a", "zed:a", "alpha:b"]
        );
    }
}
