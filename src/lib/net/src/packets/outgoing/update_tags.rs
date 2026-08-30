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
//! The contents are the tags the server itself reads, which come from the loaded datapacks: what
//! the client is told and what the server believes cannot disagree, and a pack that changes a tag
//! changes both. Only the registries whose entries have a fixed numeric id are here; the
//! datapack-driven ones (biomes, damage types, ...) carry their tags with the registry sync.

use ferrumc_macros::{packet, NetEncode};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_registry::tags::GameTags;
use std::sync::{Arc, LazyLock, RwLock};

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

/// Builds the packet from the tags as they stand.
#[must_use]
pub fn build_packet(tags: &GameTags) -> UpdateTagsPacket {
    let registries = tags
        .iter()
        .map(|(registry, tags)| {
            // Sorted so two runs of the same packs put the same bytes on the wire, which is one
            // less thing to rule out when a client disagrees about a tag.
            let mut names: Vec<&str> = tags.names().collect();
            names.sort_unstable();
            let entries = names
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
            TagRegistry {
                registry: registry.to_owned(),
                tags: LengthPrefixedVec::new(entries),
            }
        })
        .collect();

    UpdateTagsPacket {
        registries: LengthPrefixedVec::new(registries),
    }
}

/// The packet as it stands, which is rebuilt whenever the datapacks are.
///
/// It falls back to the pack the server ships with, so a connection that arrives before the packs
/// are read is still told vanilla's tags rather than none.
static CURRENT: LazyLock<RwLock<Arc<UpdateTagsPacket>>> =
    LazyLock::new(|| RwLock::new(Arc::new(build_packet(&ferrumc_registry::tags::current()))));

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
    use ferrumc_registry::tags::protocol_id;

    fn built_in() -> UpdateTagsPacket {
        build_packet(&ferrumc_registry::tags::current())
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
                let id = protocol_id("minecraft:fluid", member)
                    .unwrap_or_else(|| panic!("{member} should be a fluid"));
                assert!(sent.contains(&id), "{tag} must include {member}");
            }
        }
    }

    #[test]
    fn every_sent_registry_has_tags() {
        let packet = built_in();
        for (registry, _directory) in ferrumc_registry::tags::REGISTRIES {
            let sent = find_registry(&packet, registry);
            assert!(
                !sent.tags.data.is_empty(),
                "registry {registry} should carry at least one tag"
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

        let stack = ferrumc_datapack::ResourceManager::new(vec![
            Arc::new(ferrumc_datapack::vanilla_pack().expect("the built-in pack opens")),
            Arc::new(
                ferrumc_datapack::DirPack::open("test", dir.path().to_path_buf())
                    .expect("an openable pack"),
            ),
        ]);
        let packet = build_packet(&ferrumc_registry::tags::load(&stack));
        let logs = find_tag(find_registry(&packet, "minecraft:block"), "minecraft:logs");

        let sponge =
            protocol_id("minecraft:block", "minecraft:sponge").expect("sponge should be a block");
        assert!(logs.entries.data.iter().any(|id| id.0 == sponge));
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
