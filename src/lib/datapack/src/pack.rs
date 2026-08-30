//! A single pack, and the two shapes one arrives in: a directory or a zip.

use crate::error::DatapackError;
use crate::id::{decompose_path, Identifier};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zip::ZipArchive;

/// Where a server pack keeps its resources. Vanilla's `PackType.SERVER_DATA`; the client half of
/// the split has no meaning here, since a server never reads a resource pack.
pub const DATA_DIR: &str = "data";

/// The file at a pack's root that says what the pack is.
pub const PACK_META: &str = "pack.mcmeta";

/// A pack the server can read resources out of.
pub trait PackResources: Send + Sync {
    /// What this pack is called, for logs and for the stack that orders it.
    fn id(&self) -> &Arc<str>;

    /// A file at the pack's root, rather than under `data/`. Only `pack.mcmeta` so far.
    fn root_resource(&self, name: &str) -> Option<Vec<u8>>;

    /// The file this location addresses, if this pack carries it.
    fn resource(&self, id: &Identifier) -> Option<Vec<u8>>;

    /// Every namespace this pack has a `data/` directory for.
    fn namespaces(&self) -> &BTreeSet<String>;

    /// Every file under `data/<namespace>/<directory>`, recursively, extensions included.
    fn list(&self, namespace: &str, directory: &str, out: &mut dyn FnMut(Identifier));
}

/// A pack that is a directory on disk, which is how a datapack is usually developed.
pub struct DirPack {
    id: Arc<str>,
    root: PathBuf,
    namespaces: BTreeSet<String>,
}

impl DirPack {
    pub fn open(id: impl Into<Arc<str>>, root: PathBuf) -> Result<Self, DatapackError> {
        let data = root.join(DATA_DIR);
        let mut namespaces = BTreeSet::new();
        // A pack with no `data/` at all is empty rather than broken: vanilla reports no namespaces
        // for it and carries on.
        if let Ok(listing) = fs::read_dir(&data) {
            for entry in listing {
                let entry = entry.map_err(DatapackError::io(&data))?;
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        namespaces.insert(name.to_owned());
                    }
                }
            }
        }
        Ok(Self {
            id: id.into(),
            root,
            namespaces,
        })
    }
}

impl PackResources for DirPack {
    fn id(&self) -> &Arc<str> {
        &self.id
    }

    fn root_resource(&self, name: &str) -> Option<Vec<u8>> {
        fs::read(self.root.join(name)).ok()
    }

    fn resource(&self, id: &Identifier) -> Option<Vec<u8>> {
        fs::read(id.resolve_against(&self.root.join(DATA_DIR))?).ok()
    }

    fn namespaces(&self) -> &BTreeSet<String> {
        &self.namespaces
    }

    fn list(&self, namespace: &str, directory: &str, out: &mut dyn FnMut(Identifier)) {
        if !self.namespaces.contains(namespace) {
            return;
        }
        let Some(top) = Identifier::new(namespace, directory)
            .ok()
            .and_then(|id| id.resolve_against(&self.root.join(DATA_DIR)))
        else {
            return;
        };
        walk(&top, &mut |relative| {
            if let Ok(id) = Identifier::new(namespace, &format!("{directory}/{relative}")) {
                out(id);
            }
        });
    }
}

/// Names every file under `dir`, relative to it and with `/` separators.
fn walk(dir: &Path, out: &mut dyn FnMut(&str)) {
    fn inner(root: &Path, dir: &Path, out: &mut dyn FnMut(&str)) {
        let Ok(listing) = fs::read_dir(dir) else {
            return;
        };
        for entry in listing.flatten() {
            let path = entry.path();
            if path.is_dir() {
                inner(root, &path, out);
            } else if let Some(relative) = path.strip_prefix(root).ok().and_then(Path::to_str) {
                out(&relative.replace('\\', "/"));
            }
        }
    }
    inner(dir, dir, out);
}

/// Anything a zip can be read out of.
trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// A pack that is a zip, which is how a datapack is distributed — and how the built-in pack is
/// carried inside the executable.
pub struct ZipPack {
    id: Arc<str>,
    archive: Mutex<ZipArchive<Box<dyn ReadSeek>>>,
    namespaces: BTreeSet<String>,
}

impl ZipPack {
    /// Opens a zip on disk.
    pub fn open(id: impl Into<Arc<str>>, path: &Path) -> Result<Self, DatapackError> {
        let file = fs::File::open(path).map_err(DatapackError::io(path))?;
        Self::from_reader(id, Box::new(file))
    }

    /// Opens a zip that is part of the executable.
    pub fn embedded(id: impl Into<Arc<str>>, bytes: &'static [u8]) -> Result<Self, DatapackError> {
        Self::from_reader(id, Box::new(Cursor::new(bytes)))
    }

    fn from_reader(
        id: impl Into<Arc<str>>,
        reader: Box<dyn ReadSeek>,
    ) -> Result<Self, DatapackError> {
        let archive = ZipArchive::new(reader)?;
        let mut namespaces = BTreeSet::new();
        for name in archive.file_names() {
            if let Some(namespace) = name
                .strip_prefix(DATA_DIR)
                .and_then(|rest| rest.strip_prefix('/'))
                .and_then(|rest| rest.split('/').next())
            {
                if !namespace.is_empty() {
                    namespaces.insert(namespace.to_owned());
                }
            }
        }
        Ok(Self {
            id: id.into(),
            archive: Mutex::new(archive),
            namespaces,
        })
    }

    fn read(&self, name: &str) -> Option<Vec<u8>> {
        let mut archive = self.archive.lock().ok()?;
        let mut entry = archive.by_name(name).ok()?;
        let mut bytes = Vec::with_capacity(usize::try_from(entry.size()).unwrap_or_default());
        entry.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }
}

impl PackResources for ZipPack {
    fn id(&self) -> &Arc<str> {
        &self.id
    }

    fn root_resource(&self, name: &str) -> Option<Vec<u8>> {
        self.read(name)
    }

    fn resource(&self, id: &Identifier) -> Option<Vec<u8>> {
        // Nothing escapes an archive that is addressed by exact entry name, but a path that could
        // not name a file on disk should not name one here either.
        decompose_path(id.path())?;
        self.read(&format!("{DATA_DIR}/{}/{}", id.namespace(), id.path()))
    }

    fn namespaces(&self) -> &BTreeSet<String> {
        &self.namespaces
    }

    fn list(&self, namespace: &str, directory: &str, out: &mut dyn FnMut(Identifier)) {
        let Ok(archive) = self.archive.lock() else {
            return;
        };
        let prefix = format!("{DATA_DIR}/{namespace}/{directory}/");
        for name in archive.file_names() {
            let Some(relative) = name.strip_prefix(&prefix) else {
                continue;
            };
            if let Ok(id) = Identifier::new(namespace, &format!("{directory}/{relative}")) {
                out(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("a file has a parent")).expect("a writable dir");
        fs::write(path, contents).expect("a writable file");
    }

    #[test]
    fn a_directory_pack_reads_and_lists() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        write(root, "pack.mcmeta", "{}");
        write(
            root,
            "data/minecraft/tags/block/logs.json",
            r#"{"values":[]}"#,
        );
        write(root, "data/mypack/tags/block/deep/nested.json", "{}");

        let pack = DirPack::open("test", root.to_path_buf()).expect("an openable pack");
        assert_eq!(
            pack.namespaces()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["minecraft", "mypack"]
        );
        assert_eq!(pack.root_resource("pack.mcmeta"), Some(b"{}".to_vec()));

        let logs = Identifier::parse("minecraft:tags/block/logs.json").expect("a valid location");
        assert_eq!(pack.resource(&logs), Some(br#"{"values":[]}"#.to_vec()));

        let mut listed = Vec::new();
        pack.list("mypack", "tags/block", &mut |id| {
            listed.push(id.to_string())
        });
        assert_eq!(listed, ["mypack:tags/block/deep/nested.json"]);
    }

    #[test]
    fn a_directory_pack_refuses_a_path_that_climbs_out() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        write(root, "secret.json", "{}");
        write(root, "data/minecraft/keep.json", "{}");

        let pack = DirPack::open("test", root.to_path_buf()).expect("an openable pack");
        let escaping = Identifier::new("minecraft", "../../secret.json").expect("a valid location");
        assert_eq!(pack.resource(&escaping), None);
    }

    #[test]
    fn the_built_in_pack_is_a_readable_zip() {
        let pack = crate::vanilla_pack().expect("the built-in pack should open");
        assert!(pack.namespaces().contains("minecraft"));
        assert!(pack.root_resource(PACK_META).is_some());

        let logs = Identifier::parse("minecraft:tags/block/logs.json").expect("a valid location");
        let bytes = pack.resource(&logs).expect("vanilla has a logs tag");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("a tag is json");
        assert!(json["values"]
            .as_array()
            .is_some_and(|values| values.iter().any(|v| v == "#minecraft:logs_that_burn")));

        let mut count = 0;
        pack.list("minecraft", "tags/block", &mut |_| count += 1);
        assert!(
            count > 200,
            "vanilla has hundreds of block tags, found {count}"
        );
    }
}
