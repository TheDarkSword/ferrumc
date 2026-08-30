//! Tags: named sets over a registry, and the only thing in the game that merges across packs
//! instead of being overridden by the last one to declare it.
//!
//! A tag file lists elements by id, other tags by `#id`, and either of those may be marked
//! optional. Tags refer to each other freely, so building them is a dependency order followed by
//! a flattening: what a query sees is a plain set, never a reference to follow.

use crate::id::Identifier;
use crate::manager::{FileToId, ResourceManager};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use tracing::{error, warn};

/// One line of a tag file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagEntry {
    /// The element, or the tag, this line names.
    pub id: Identifier,
    /// Whether the line names a tag rather than an element.
    pub is_tag: bool,
    /// A line that is not required is skipped when what it names is not there; a required one
    /// that is missing sinks the whole tag.
    pub required: bool,
}

impl TagEntry {
    fn parse(value: &Value) -> Option<Self> {
        match value {
            Value::String(id) => Self::from_id(id, true),
            object => {
                let id = object.get("id")?.as_str()?;
                let required = object
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(true);
                Self::from_id(id, required)
            }
        }
    }

    fn from_id(id: &str, required: bool) -> Option<Self> {
        let (is_tag, id) = match id.strip_prefix('#') {
            Some(tag) => (true, tag),
            None => (false, id),
        };
        Some(Self {
            id: Identifier::parse(id).ok()?,
            is_tag,
            required,
        })
    }
}

impl std::fmt::Display for TagEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_tag {
            f.write_str("#")?;
        }
        write!(f, "{}", self.id)?;
        if !self.required {
            f.write_str("?")?;
        }
        Ok(())
    }
}

/// The lines a tag was declared with, in the order the packs contributed them.
#[derive(Default)]
pub struct RawTags {
    entries: BTreeMap<Identifier, Vec<(TagEntry, Arc<str>)>>,
}

impl RawTags {
    /// Reads every tag file in `directory` across the whole stack.
    ///
    /// A pack that declares a tag adds to what the packs below it declared, unless it says
    /// `"replace": true`, which drops everything declared so far.
    pub fn load(manager: &ResourceManager, directory: &str) -> Self {
        let mut tags = Self::default();
        let files = FileToId::json(directory);
        for (id, stack) in files.list_stacks(manager) {
            let entries = tags.entries.entry(id.clone()).or_default();
            for resource in stack {
                match parse_tag_file(&resource.data) {
                    Ok((lines, replace)) => {
                        if replace {
                            entries.clear();
                        }
                        entries.extend(
                            lines
                                .into_iter()
                                .map(|entry| (entry, Arc::clone(&resource.source))),
                        );
                    }
                    Err(e) => error!(
                        "couldn't read tag list {id} from data pack {}: {e}",
                        resource.source
                    ),
                }
            }
        }
        tags
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Flattens every tag into the elements it holds.
    ///
    /// `lookup` turns an element's id into its index in the registry the tag is over, and
    /// `element_count` is how many of those there are.
    pub fn build(
        &self,
        element_count: usize,
        lookup: impl Fn(&Identifier) -> Option<u32>,
    ) -> TagRegistry {
        let mut registry = TagRegistry::new(element_count);
        for id in self.order() {
            let Some(entries) = self.entries.get(&id) else {
                continue;
            };
            match self.resolve(entries, &lookup, &registry) {
                Ok(elements) => registry.insert(id, elements),
                Err(missing) => error!(
                    "couldn't load tag {id} as it is missing following references: {}",
                    missing
                        .iter()
                        .map(|(entry, source)| format!("{entry} (from {source})"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        }
        registry
    }

    /// Turns one tag's lines into elements, or reports the required ones that were not there.
    fn resolve(
        &self,
        entries: &[(TagEntry, Arc<str>)],
        lookup: &impl Fn(&Identifier) -> Option<u32>,
        built: &TagRegistry,
    ) -> Result<Vec<u32>, Vec<(TagEntry, Arc<str>)>> {
        // Insertion-ordered and deduplicated, as vanilla's `LinkedHashSet` is: what order a tag
        // holds its elements in is visible wherever one is picked by index.
        let mut elements: Vec<u32> = Vec::new();
        let mut seen = HashSet::new();
        let mut missing = Vec::new();

        for (entry, source) in entries {
            let found = if entry.is_tag {
                built.get(&entry.id).map(|tag| {
                    for &element in built.elements(tag) {
                        if seen.insert(element) {
                            elements.push(element);
                        }
                    }
                })
            } else {
                lookup(&entry.id).map(|element| {
                    if seen.insert(element) {
                        elements.push(element);
                    }
                })
            };
            if found.is_none() && entry.required {
                missing.push((entry.clone(), Arc::clone(source)));
            }
        }

        if missing.is_empty() {
            Ok(elements)
        } else {
            Err(missing)
        }
    }

    /// Every tag, with the ones it refers to ahead of it.
    ///
    /// A reference that would close a cycle is dropped, so a datapack that ties two tags together
    /// still loads with whichever of them was reached first, rather than neither.
    fn order(&self) -> Vec<Identifier> {
        let mut ordered = Vec::with_capacity(self.entries.len());
        let mut done = HashSet::new();
        let mut on_the_path = HashSet::new();
        for id in self.entries.keys() {
            self.visit(id, &mut ordered, &mut done, &mut on_the_path);
        }
        ordered
    }

    fn visit(
        &self,
        id: &Identifier,
        ordered: &mut Vec<Identifier>,
        done: &mut HashSet<Identifier>,
        on_the_path: &mut HashSet<Identifier>,
    ) {
        if done.contains(id) || !on_the_path.insert(id.clone()) {
            return;
        }
        if let Some(entries) = self.entries.get(id) {
            for (entry, _) in entries {
                if entry.is_tag {
                    self.visit(&entry.id, ordered, done, on_the_path);
                }
            }
        }
        on_the_path.remove(id);
        if done.insert(id.clone()) {
            ordered.push(id.clone());
        }
    }
}

/// Reads a tag file: the lines it declares, and whether it drops what came before it.
fn parse_tag_file(bytes: &[u8]) -> Result<(Vec<TagEntry>, bool), serde_json::Error> {
    let file: Value = serde_json::from_slice(bytes)?;
    let replace = file
        .get("replace")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let values = file
        .get("values")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let entries = values
        .iter()
        .filter_map(|value| {
            let entry = TagEntry::parse(value);
            if entry.is_none() {
                warn!("ignoring unreadable tag entry {value}");
            }
            entry
        })
        .collect();
    Ok((entries, replace))
}

/// A handle to one tag, resolved once and then asked many times.
///
/// Vanilla's `TagKey`, minus the registry: a registry has its own set of tags here, so a handle
/// only means anything against the one it came from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TagId(usize);

/// Every tag over one registry, flattened.
pub struct TagRegistry {
    /// Keyed by the whole `namespace:path`, so a lookup by a name that already carries its
    /// namespace costs nothing beyond the hash.
    by_name: HashMap<Box<str>, TagId>,
    /// Membership, one bit per element, so a query is a bit test.
    bits: Vec<Box<[u64]>>,
    /// The same members in the order the tag declared them, for the callers that walk a tag or
    /// pick out of it by index.
    ordered: Vec<Box<[u32]>>,
    element_count: usize,
}

impl TagRegistry {
    pub fn new(element_count: usize) -> Self {
        Self {
            by_name: HashMap::new(),
            bits: Vec::new(),
            ordered: Vec::new(),
            element_count,
        }
    }

    fn insert(&mut self, name: Identifier, elements: Vec<u32>) {
        let mut bits = vec![0u64; self.element_count.div_ceil(64)].into_boxed_slice();
        for &element in &elements {
            let element = element as usize;
            if element < self.element_count {
                bits[element / 64] |= 1 << (element % 64);
            }
        }
        self.by_name
            .insert(name.as_str().into(), TagId(self.bits.len()));
        self.bits.push(bits);
        self.ordered.push(elements.into_boxed_slice());
    }

    /// The handle for this tag, if it exists.
    pub fn get(&self, name: &Identifier) -> Option<TagId> {
        self.by_name.get(name.as_str()).copied()
    }

    /// The handle for this tag by name. A name without a namespace is read as `minecraft:`.
    pub fn get_by_name(&self, name: &str) -> Option<TagId> {
        if name.contains(':') {
            self.by_name.get(name).copied()
        } else {
            self.by_name.get(&*format!("minecraft:{name}")).copied()
        }
    }

    /// Whether the tag holds this element.
    pub fn contains(&self, tag: TagId, element: u32) -> bool {
        let element = element as usize;
        element < self.element_count && self.bits[tag.0][element / 64] & (1 << (element % 64)) != 0
    }

    /// The tag's elements, in the order it declared them.
    pub fn elements(&self, tag: TagId) -> &[u32] {
        &self.ordered[tag.0]
    }

    /// Every tag's name.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.by_name.keys().map(|name| &**name)
    }

    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A registry of four things, named a through d, indexed in that order.
    fn lookup(id: &Identifier) -> Option<u32> {
        ["a", "b", "c", "d"]
            .iter()
            .position(|name| *name == id.path())
            .map(|index| index as u32)
    }

    fn raw(files: &[(&str, &str)]) -> RawTags {
        let mut tags = RawTags::default();
        for (name, json) in files {
            let (entries, _) = parse_tag_file(json.as_bytes()).expect("a readable tag file");
            tags.entries.insert(
                Identifier::parse(name).expect("a valid location"),
                entries
                    .into_iter()
                    .map(|entry| (entry, Arc::from("test")))
                    .collect(),
            );
        }
        tags
    }

    fn built(files: &[(&str, &str)]) -> TagRegistry {
        raw(files).build(4, lookup)
    }

    #[test]
    fn a_tag_holds_what_it_names() {
        let tags = built(&[(
            "minecraft:letters",
            r#"{"values":["minecraft:a","minecraft:c"]}"#,
        )]);
        let letters = tags
            .get_by_name("minecraft:letters")
            .expect("the tag built");
        assert!(tags.contains(letters, 0));
        assert!(!tags.contains(letters, 1));
        assert!(tags.contains(letters, 2));
        assert_eq!(tags.elements(letters), [0, 2]);
    }

    #[test]
    fn a_tag_that_names_another_takes_its_contents() {
        let tags = built(&[
            ("minecraft:inner", r#"{"values":["minecraft:a"]}"#),
            (
                "minecraft:outer",
                r##"{"values":["#minecraft:inner","minecraft:b"]}"##,
            ),
        ]);
        let outer = tags.get_by_name("minecraft:outer").expect("the tag built");
        assert_eq!(tags.elements(outer), [0, 1]);
    }

    #[test]
    fn an_element_named_twice_appears_once() {
        let tags = built(&[
            ("minecraft:inner", r#"{"values":["minecraft:a"]}"#),
            (
                "minecraft:outer",
                r##"{"values":["minecraft:a","#minecraft:inner"]}"##,
            ),
        ]);
        let outer = tags.get_by_name("minecraft:outer").expect("the tag built");
        assert_eq!(tags.elements(outer), [0]);
    }

    #[test]
    fn a_missing_required_reference_sinks_the_tag_and_what_depends_on_it() {
        let tags = built(&[
            ("minecraft:broken", r#"{"values":["minecraft:nowhere"]}"#),
            (
                "minecraft:depends",
                r##"{"values":["#minecraft:broken","minecraft:a"]}"##,
            ),
        ]);
        assert!(tags.get_by_name("minecraft:broken").is_none());
        assert!(tags.get_by_name("minecraft:depends").is_none());
    }

    #[test]
    fn an_optional_reference_that_is_missing_is_skipped() {
        let tags = built(&[(
            "minecraft:letters",
            r#"{"values":[{"id":"minecraft:nowhere","required":false},"minecraft:a"]}"#,
        )]);
        let letters = tags
            .get_by_name("minecraft:letters")
            .expect("the tag built");
        assert_eq!(tags.elements(letters), [0]);
    }

    #[test]
    fn declaring_a_tag_twice_adds_to_it_unless_it_replaces() {
        let mut tags = RawTags::default();
        let name = Identifier::parse("minecraft:letters").expect("a valid location");
        let entries = tags.entries.entry(name.clone()).or_default();
        for (json, source) in [
            (r#"{"values":["minecraft:a"]}"#, "first"),
            (r#"{"values":["minecraft:b"]}"#, "second"),
        ] {
            let (lines, replace) = parse_tag_file(json.as_bytes()).expect("a readable tag file");
            if replace {
                entries.clear();
            }
            entries.extend(lines.into_iter().map(|e| (e, Arc::from(source))));
        }
        let built = tags.build(4, lookup);
        let letters = built.get(&name).expect("the tag built");
        assert_eq!(built.elements(letters), [0, 1]);

        // The same again, with the second pack replacing rather than adding.
        let mut tags = RawTags::default();
        let entries = tags.entries.entry(name.clone()).or_default();
        for (json, source) in [
            (r#"{"values":["minecraft:a"]}"#, "first"),
            (r#"{"replace":true,"values":["minecraft:b"]}"#, "second"),
        ] {
            let (lines, replace) = parse_tag_file(json.as_bytes()).expect("a readable tag file");
            if replace {
                entries.clear();
            }
            entries.extend(lines.into_iter().map(|e| (e, Arc::from(source))));
        }
        let built = tags.build(4, lookup);
        let letters = built.get(&name).expect("the tag built");
        assert_eq!(built.elements(letters), [1]);
    }

    #[test]
    fn tags_that_require_each_other_both_go() {
        let tags = built(&[
            (
                "minecraft:one",
                r##"{"values":["minecraft:a","#minecraft:two"]}"##,
            ),
            (
                "minecraft:two",
                r##"{"values":["minecraft:b","#minecraft:one"]}"##,
            ),
        ]);
        // The cycle is broken somewhere, so whichever is built first cannot see the other and
        // fails on a reference it required; the other then fails on it. Losing both is what
        // vanilla does with the same files. What matters here is that it ends rather than
        // chasing the cycle.
        assert!(tags.is_empty());
    }

    #[test]
    fn a_cycle_of_optional_references_survives() {
        let tags = built(&[
            (
                "minecraft:one",
                r##"{"values":["minecraft:a",{"id":"#minecraft:two","required":false}]}"##,
            ),
            (
                "minecraft:two",
                r##"{"values":["minecraft:b",{"id":"#minecraft:one","required":false}]}"##,
            ),
        ]);
        let one = tags.get_by_name("minecraft:one").expect("the tag built");
        let two = tags.get_by_name("minecraft:two").expect("the tag built");
        // One of them is built first and sees nothing of the other; the second then takes what
        // the first holds.
        assert!(tags.contains(one, 0));
        assert!(tags.contains(two, 1));
    }
}
