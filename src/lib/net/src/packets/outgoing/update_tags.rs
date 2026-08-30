//! The clientbound **Update Tags** packet (configuration state).
//!
//! Tags group registry entries (blocks, items, fluids, ...) under named sets like
//! `minecraft:water` or `minecraft:lava`. The vanilla client relies on these heavily and *will not
//! synthesise them itself*: fluid rendering picks the lava vs water sprites from the
//! `minecraft:fluid` / `minecraft:lava` / `minecraft:water` tags, the translucent-vs-opaque render
//! layer for water comes from the water tag, and entity fluid physics (the "resistance" you feel
//! wading through water) is gated on the water tag too. If this packet is never sent, the client
//! treats every fluid tag as empty, so lava falls back to the water sprite (but stays opaque) and
//! water applies no movement resistance.
//!
//! The contents come from the loaded datapacks, so a pack that changes a tag changes what the
//! client is told as well as what the server believes. Names are turned into the numeric ids the
//! wire carries with `assets/data/registries.json`.
//!
//! Only the built-in (non-datapack) registries are sent here; the dynamic registries (biomes,
//! damage types, ...) carry their tags through the registry sync instead.

use ferrumc_datapack::tag::RawTags;
use ferrumc_datapack::ResourceManager;
use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

const REGISTRIES_JSON: &str = include_str!("../../../../../../assets/data/registries.json");

/// The registries whose tags are sent here, paired with the directory a pack keeps them in.
///
/// These are the built-in registries whose entries have stable numeric protocol ids in
/// `registries.json`. Datapack-driven registries (worldgen/biome, damage_type, enchantment, ...)
/// are intentionally omitted: their tags travel with the synced registry data, not here.
const SENT_REGISTRIES: &[(&str, &str)] = &[
    ("tags/block", "minecraft:block"),
    ("tags/item", "minecraft:item"),
    ("tags/fluid", "minecraft:fluid"),
    ("tags/entity_type", "minecraft:entity_type"),
    ("tags/game_event", "minecraft:game_event"),
    (
        "tags/point_of_interest_type",
        "minecraft:point_of_interest_type",
    ),
];

#[derive(NetEncode)]
#[packet(packet_id = "update_tags", state = "configuration")]
pub struct UpdateTagsPacket {
    pub registries: LengthPrefixedVec<TagRegistry>,
}

#[derive(NetEncode)]
pub struct TagRegistry {
    /// Registry identifier, e.g. `minecraft:fluid`.
    pub registry: String,
    pub tags: LengthPrefixedVec<TagEntry>,
}

#[derive(NetEncode)]
pub struct TagEntry {
    /// Tag identifier, e.g. `minecraft:lava`.
    pub name: String,
    /// Numeric protocol ids of the entries in this tag.
    pub entries: LengthPrefixedVec<VarInt>,
}

/// Builds `entry name -> protocol id` lookups for every registry tags are sent for.
fn registry_ids() -> HashMap<&'static str, HashMap<String, i32>> {
    let registries: Value =
        serde_json::from_str(REGISTRIES_JSON).expect("registries.json should be valid JSON");
    let mut out = HashMap::new();

    for (_directory, registry_id) in SENT_REGISTRIES {
        let Some(entries) = registries
            .get(registry_id)
            .and_then(|r| r.get("entries"))
            .and_then(Value::as_object)
        else {
            continue;
        };

        let mut id_map = HashMap::with_capacity(entries.len());
        for (entry_name, info) in entries {
            if let Some(id) = info.get("protocol_id").and_then(Value::as_i64) {
                id_map.insert(entry_name.clone(), id as i32);
            }
        }
        out.insert(*registry_id, id_map);
    }

    out
}

/// Builds the packet from what the loaded packs declare.
pub fn build_packet(manager: &ResourceManager) -> UpdateTagsPacket {
    let id_maps = registry_ids();
    let mut registries = Vec::with_capacity(SENT_REGISTRIES.len());

    for (directory, registry_id) in SENT_REGISTRIES {
        let Some(id_map) = id_maps.get(registry_id) else {
            continue;
        };
        // The ids the wire carries are the registry's own, so they are what the tags resolve to
        // directly rather than something looked up again afterwards.
        let element_count = id_map.values().max().map_or(0, |max| *max as usize + 1);
        let tags = RawTags::load(manager, directory).build(element_count, |id| {
            id_map
                .get(id.as_str())
                .and_then(|id| u32::try_from(*id).ok())
        });

        // Sorted so two runs of the same packs put the same bytes on the wire, which is one less
        // thing to rule out when a client disagrees about a tag.
        let mut names: Vec<&str> = tags.names().collect();
        names.sort_unstable();
        let tag_entries = names
            .into_iter()
            .filter_map(|name| {
                let tag = tags.get_by_name(name)?;
                Some(TagEntry {
                    name: name.to_owned(),
                    entries: LengthPrefixedVec::new(
                        tags.elements(tag)
                            .iter()
                            .filter_map(|id| i32::try_from(*id).ok().map(VarInt::new))
                            .collect(),
                    ),
                })
            })
            .collect();

        registries.push(TagRegistry {
            registry: (*registry_id).to_string(),
            tags: LengthPrefixedVec::new(tag_entries),
        });
    }

    UpdateTagsPacket {
        registries: LengthPrefixedVec::new(registries),
    }
}

/// The packet as it stands, which is rebuilt whenever the datapacks are.
///
/// It falls back to the pack the server ships with, so a connection that arrives before the packs
/// are read is still told vanilla's tags rather than none.
static CURRENT: LazyLock<RwLock<Arc<UpdateTagsPacket>>> = LazyLock::new(|| {
    let built_in = ferrumc_datapack::vanilla_pack()
        .map(|pack| ResourceManager::new(vec![Arc::new(pack)]))
        .map_or_else(
            |e| {
                tracing::error!("could not read the built-in tags: {e}");
                UpdateTagsPacket {
                    registries: LengthPrefixedVec::new(Vec::new()),
                }
            },
            |manager| build_packet(&manager),
        );
    RwLock::new(Arc::new(built_in))
});

/// The packet to send to a connecting client.
#[must_use]
pub fn current() -> Arc<UpdateTagsPacket> {
    CURRENT
        .read()
        .expect("the tag packet is never held across a panic")
        .clone()
}

/// Replaces it, which is what loading or reloading datapacks does.
pub fn set(packet: Arc<UpdateTagsPacket>) {
    *CURRENT
        .write()
        .expect("the tag packet is never held across a panic") = packet;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in() -> UpdateTagsPacket {
        let pack = ferrumc_datapack::vanilla_pack().expect("the built-in pack opens");
        build_packet(&ResourceManager::new(vec![Arc::new(pack)]))
    }

    fn find_registry<'a>(packet: &'a UpdateTagsPacket, id: &str) -> &'a TagRegistry {
        packet
            .registries
            .data
            .iter()
            .find(|r| r.registry == id)
            .unwrap_or_else(|| panic!("registry {id} should be present"))
    }

    fn find_tag<'a>(registry: &'a TagRegistry, name: &str) -> &'a TagEntry {
        registry
            .tags
            .data
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("tag {name} should be present"))
    }

    /// The two tags the client tells water and lava apart by. Which number each fluid has moves
    /// with the version, so the ids are looked up rather than written down.
    #[test]
    fn fluid_tags_carry_the_fluids_they_name() {
        let packet = built_in();
        let fluid = find_registry(&packet, "minecraft:fluid");
        let ids = registry_ids();
        let fluid_ids = ids
            .get("minecraft:fluid")
            .expect("the fluid registry is in registries.json");

        for (tag, members) in [
            (
                "minecraft:lava",
                ["minecraft:lava", "minecraft:flowing_lava"],
            ),
            (
                "minecraft:water",
                ["minecraft:water", "minecraft:flowing_water"],
            ),
        ] {
            let sent: Vec<i32> = find_tag(fluid, tag)
                .entries
                .data
                .iter()
                .map(|v| v.0)
                .collect();
            for member in members {
                let id = fluid_ids
                    .get(member)
                    .unwrap_or_else(|| panic!("{member} should be a fluid"));
                assert!(sent.contains(id), "{tag} must include {member}");
            }
        }
    }

    #[test]
    fn every_sent_registry_has_tags() {
        let packet = built_in();
        for (_directory, registry_id) in SENT_REGISTRIES {
            let registry = find_registry(&packet, registry_id);
            assert!(
                !registry.tags.data.is_empty(),
                "registry {registry_id} should carry at least one tag"
            );
        }
    }

    /// The point of reading tags from the packs rather than baking them in: what the client is
    /// told follows a datapack.
    #[test]
    fn a_datapack_changes_what_the_client_is_told() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let tag = dir.path().join("data/minecraft/tags/block/logs.json");
        std::fs::create_dir_all(tag.parent().expect("a file has a parent"))
            .expect("a writable dir");
        std::fs::write(&tag, r#"{"values":["minecraft:sponge"]}"#).expect("a writable file");

        let stack = ResourceManager::new(vec![
            Arc::new(ferrumc_datapack::vanilla_pack().expect("the built-in pack opens")),
            Arc::new(
                ferrumc_datapack::DirPack::open("test", dir.path().to_path_buf())
                    .expect("an openable pack"),
            ),
        ]);
        let packet = build_packet(&stack);
        let logs = find_tag(find_registry(&packet, "minecraft:block"), "minecraft:logs");

        let ids = registry_ids();
        let sponge = ids
            .get("minecraft:block")
            .and_then(|blocks| blocks.get("minecraft:sponge"))
            .expect("sponge should be a block");
        assert!(logs.entries.data.iter().any(|id| id.0 == *sponge));
    }

    #[test]
    fn packet_encodes_without_error() {
        use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
        use ferrumc_net_codec::version::ProtocolVersion;
        use std::io::Cursor;

        // The packet is sent during configuration; if it fails to encode (or encodes to nonsense)
        // every client would be disconnected mid-handshake. Encode it the same way the wire path
        // does (length-prefixed) and sanity-check the output.
        let packet = built_in();
        let mut buf = Cursor::new(Vec::new());
        packet
            .encode(&mut buf, &NetEncodeOpts::packet(ProtocolVersion::CURRENT))
            .expect("update_tags must encode");
        let bytes = buf.into_inner();
        assert!(
            bytes.len() > 2,
            "encoded packet should be non-trivial, got {} bytes",
            bytes.len()
        );
    }
}
