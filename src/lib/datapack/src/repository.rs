//! Finding the packs there are, and deciding which of them are on.

use crate::error::DatapackError;
use crate::manager::ResourceManager;
use crate::meta::{PackCompatibility, PackMetadata, CURRENT_PACK_FORMAT};
use crate::pack::{DirPack, PackResources, ZipPack, PACK_META};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

/// The id the pack that ships with the server goes by, as vanilla names it.
pub const VANILLA_PACK_ID: &str = "vanilla";

/// Where a pack came from, which decides whether it is on unless told otherwise.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PackSource {
    /// The pack inside the executable.
    BuiltIn,
    /// A pack a player dropped in the datapack directory.
    World,
}

/// A pack that was found, opened and asked what it is.
pub struct DiscoveredPack {
    pub id: String,
    pub meta: PackMetadata,
    pub compatibility: PackCompatibility,
    pub source: PackSource,
    resources: Arc<dyn PackResources>,
}

impl DiscoveredPack {
    /// Opens a pack and reads its `pack.mcmeta`. A pack without one is not a pack.
    fn read(
        id: String,
        source: PackSource,
        resources: Arc<dyn PackResources>,
    ) -> Result<Self, DatapackError> {
        let bytes = resources
            .root_resource(PACK_META)
            .ok_or_else(|| DatapackError::NoMetadata(id.clone()))?;
        let meta = PackMetadata::parse(&bytes).map_err(|source| DatapackError::Metadata {
            pack: id.clone(),
            source,
        })?;
        Ok(Self {
            compatibility: PackCompatibility::of(meta.supported_formats, CURRENT_PACK_FORMAT),
            id,
            meta,
            source,
            resources,
        })
    }
}

/// Every pack the server knows about, and which of them are on.
pub struct PackRepository {
    datapack_dir: PathBuf,
    /// Keyed by id, which also sorts the folder packs among themselves.
    available: BTreeMap<String, Arc<DiscoveredPack>>,
    /// Lowest priority first: the built-in pack, then whatever overrides it.
    selected: Vec<String>,
}

impl PackRepository {
    /// Reads the built-in pack and everything in `datapack_dir`.
    pub fn discover(datapack_dir: PathBuf) -> Result<Self, DatapackError> {
        let mut repository = Self {
            datapack_dir,
            available: BTreeMap::new(),
            selected: Vec::new(),
        };
        repository.reload()?;
        Ok(repository)
    }

    /// Looks again, keeping any selection whose packs are still there and picking up new ones.
    ///
    /// This is what `/reload` runs.
    pub fn reload(&mut self) -> Result<(), DatapackError> {
        let previously_selected = std::mem::take(&mut self.selected);
        self.available.clear();

        let vanilla = Arc::new(crate::vanilla_pack()?) as Arc<dyn PackResources>;
        let vanilla =
            DiscoveredPack::read(VANILLA_PACK_ID.to_owned(), PackSource::BuiltIn, vanilla)?;
        self.available.insert(vanilla.id.clone(), Arc::new(vanilla));

        // A missing directory means no packs, not a failure; it is created on the first launch.
        if let Err(e) = fs::create_dir_all(&self.datapack_dir) {
            warn!(
                "could not create the datapack directory {}: {e}",
                self.datapack_dir.display()
            );
        }
        for pack in self.discover_folder_packs() {
            self.available.insert(pack.id.clone(), Arc::new(pack));
        }

        // The built-in pack sits at the bottom of the stack; everything found overrides it. A pack
        // that was on and is still there stays where it was, so a reload does not reshuffle.
        let mut selected = vec![VANILLA_PACK_ID.to_owned()];
        for id in previously_selected {
            if id != VANILLA_PACK_ID && self.available.contains_key(&id) {
                selected.push(id);
            }
        }
        for (id, pack) in &self.available {
            if pack.source == PackSource::World && !selected.contains(id) {
                info!("found new data pack {id}, loading it automatically");
                selected.push(id.clone());
            }
        }
        for id in &selected {
            if let Some(pack) = self.available.get(id) {
                if !pack.compatibility.is_compatible() {
                    warn!(
                        "data pack {id} was made for a different version of the game ({:?}); \
                         it may not load correctly",
                        pack.compatibility
                    );
                }
            }
        }
        self.selected = selected;
        Ok(())
    }

    fn discover_folder_packs(&self) -> Vec<DiscoveredPack> {
        let Ok(listing) = fs::read_dir(&self.datapack_dir) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for entry in listing.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // Vanilla names a discovered pack after the file it came out of.
            let id = format!("file/{name}");
            match open_folder_entry(&id, &path) {
                Ok(Some(pack)) => found.push(pack),
                Ok(None) => info!("found non-pack entry '{}', ignoring", path.display()),
                Err(e) => warn!("ignoring data pack '{}': {e}", path.display()),
            }
        }
        found
    }

    pub fn available(&self) -> impl Iterator<Item = &Arc<DiscoveredPack>> {
        self.available.values()
    }

    pub fn get(&self, id: &str) -> Option<&Arc<DiscoveredPack>> {
        self.available.get(id)
    }

    /// The packs that are on, lowest priority first.
    pub fn selected(&self) -> &[String] {
        &self.selected
    }

    /// Turns a set of packs on, in the order given. Unknown ids are dropped with a warning.
    pub fn set_selected(&mut self, ids: impl IntoIterator<Item = String>) {
        self.selected = ids
            .into_iter()
            .filter(|id| {
                let known = self.available.contains_key(id);
                if !known {
                    warn!("missing data pack {id}");
                }
                known
            })
            .collect();
    }

    /// The stack the selected packs form, which is what everything else reads through.
    pub fn open(&self) -> ResourceManager {
        ResourceManager::new(
            self.selected
                .iter()
                .filter_map(|id| self.available.get(id))
                .map(|pack| Arc::clone(&pack.resources))
                .collect(),
        )
    }
}

/// Reads one entry of the datapack directory, if it is a pack at all.
fn open_folder_entry(id: &str, path: &Path) -> Result<Option<DiscoveredPack>, DatapackError> {
    let resources: Arc<dyn PackResources> = if path.is_dir() {
        if !path.join(PACK_META).is_file() {
            return Ok(None);
        }
        Arc::new(DirPack::open(id, path.to_path_buf())?)
    } else if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
    {
        Arc::new(ZipPack::open(id, path)?)
    } else {
        return Ok(None);
    };
    DiscoveredPack::read(id.to_owned(), PackSource::World, resources).map(Some)
}
