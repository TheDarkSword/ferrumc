//! The stack a set of packs forms, and reading through it.
//!
//! Packs are held lowest priority first, so a later pack shadows an earlier one for the same
//! location. Some things want only the winner — a loot table is one file — and some want every
//! copy, since a tag is merged across every pack that declares it.

use crate::id::Identifier;
use crate::pack::PackResources;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// A file, and which pack it came out of.
#[derive(Clone)]
pub struct Resource {
    pub source: Arc<str>,
    pub data: Vec<u8>,
}

/// Every selected pack, in the order they override each other.
#[derive(Clone, Default)]
pub struct ResourceManager {
    /// Lowest priority first, so the last pack to declare a location wins it.
    packs: Vec<Arc<dyn PackResources>>,
}

impl ResourceManager {
    pub fn new(packs: Vec<Arc<dyn PackResources>>) -> Self {
        Self { packs }
    }

    /// The winning copy of this location.
    pub fn get(&self, id: &Identifier) -> Option<Resource> {
        self.packs.iter().rev().find_map(|pack| {
            pack.resource(id).map(|data| Resource {
                source: Arc::clone(pack.id()),
                data,
            })
        })
    }

    /// Every copy of this location, lowest priority first, for the things that merge rather than
    /// override.
    pub fn get_stack(&self, id: &Identifier) -> Vec<Resource> {
        self.packs
            .iter()
            .filter_map(|pack| {
                pack.resource(id).map(|data| Resource {
                    source: Arc::clone(pack.id()),
                    data,
                })
            })
            .collect()
    }

    /// Every location under `directory` that `filter` accepts, across every pack.
    pub fn list(
        &self,
        directory: &str,
        filter: impl Fn(&Identifier) -> bool,
    ) -> BTreeMap<Identifier, Resource> {
        self.locations(directory, filter)
            .into_iter()
            .filter_map(|id| self.get(&id).map(|resource| (id, resource)))
            .collect()
    }

    /// The same listing, with every pack's copy of each location rather than the winner.
    pub fn list_stacks(
        &self,
        directory: &str,
        filter: impl Fn(&Identifier) -> bool,
    ) -> BTreeMap<Identifier, Vec<Resource>> {
        self.locations(directory, filter)
            .into_iter()
            .map(|id| {
                let stack = self.get_stack(&id);
                (id, stack)
            })
            .filter(|(_, stack)| !stack.is_empty())
            .collect()
    }

    /// Every namespace any selected pack carries.
    pub fn namespaces(&self) -> BTreeSet<&str> {
        self.packs
            .iter()
            .flat_map(|pack| pack.namespaces().iter().map(String::as_str))
            .collect()
    }

    pub fn packs(&self) -> &[Arc<dyn PackResources>] {
        &self.packs
    }

    fn locations(
        &self,
        directory: &str,
        filter: impl Fn(&Identifier) -> bool,
    ) -> BTreeSet<Identifier> {
        let mut found = BTreeSet::new();
        for pack in &self.packs {
            for namespace in pack.namespaces() {
                pack.list(namespace, directory, &mut |id| {
                    if filter(&id) {
                        found.insert(id);
                    }
                });
            }
        }
        found
    }
}

/// The pairing of a directory with the extension its files carry, which is what turns a file
/// location into the id of the thing inside it.
///
/// Vanilla's `FileToIdConverter`: `tags/block` plus `.json` maps
/// `minecraft:tags/block/logs.json` to `minecraft:logs` and back.
pub struct FileToId {
    prefix: String,
    extension: &'static str,
}

impl FileToId {
    pub fn json(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            extension: ".json",
        }
    }

    /// The file the thing of this id lives in.
    pub fn id_to_file(&self, id: &Identifier) -> Option<Identifier> {
        id.with_path(&format!("{}/{}{}", self.prefix, id.path(), self.extension))
            .ok()
    }

    /// The id of the thing this file holds.
    pub fn file_to_id(&self, file: &Identifier) -> Option<Identifier> {
        file.path()
            .strip_prefix(&self.prefix)?
            .strip_prefix('/')?
            .strip_suffix(self.extension)
            .and_then(|path| file.with_path(path).ok())
    }

    /// Every file in this directory, across the stack, keyed by the id it holds.
    pub fn list_stacks(&self, manager: &ResourceManager) -> BTreeMap<Identifier, Vec<Resource>> {
        manager
            .list_stacks(&self.prefix, |id| self.matches(id))
            .into_iter()
            .filter_map(|(file, stack)| Some((self.file_to_id(&file)?, stack)))
            .collect()
    }

    /// The same, keeping only the pack that won each id.
    pub fn list(&self, manager: &ResourceManager) -> BTreeMap<Identifier, Resource> {
        manager
            .list(&self.prefix, |id| self.matches(id))
            .into_iter()
            .filter_map(|(file, resource)| Some((self.file_to_id(&file)?, resource)))
            .collect()
    }

    fn matches(&self, id: &Identifier) -> bool {
        id.path().ends_with(self.extension)
    }
}
