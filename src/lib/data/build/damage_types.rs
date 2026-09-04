use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::{collections::BTreeMap, fs};

/// Where the packs keep what a kind of damage is, and which groups it belongs to.
const DAMAGE_TYPES: &str = "../../../assets/extracted/26.2/data/minecraft/damage_type";
const DAMAGE_TAGS: &str = "../../../assets/extracted/26.2/data/minecraft/tags/damage_type";

/// Where the payload each version's client is actually sent lives.
const REGISTRY_PACKETS: &str = "../../../assets/data/registry_packets";

/// The supported versions, in the order of `ProtocolVersion::ALL`.
const REGISTRY_PAYLOADS: [&str; 10] = [
    "1.21", "1.21.2", "1.21.4", "1.21.5", "1.21.6", "1.21.8", "1.21.9", "1.21.11", "26.1", "26.2",
];

/// The groups the damage pipeline asks about. Each is a tag the packs define, and each decides
/// one step: whether armour softens the blow, whether resistance does, whether the blow lands at
/// all during the moments after the last one.
const ASKED_ABOUT: &[(&str, &str)] = &[
    ("bypasses_armor", "goes_through_armour"),
    ("bypasses_effects", "goes_through_effects"),
    ("bypasses_resistance", "goes_through_resistance"),
    ("bypasses_enchantments", "goes_through_enchantments"),
    ("bypasses_invulnerability", "goes_through_invulnerability"),
    ("bypasses_cooldown", "goes_through_the_cooldown"),
    ("is_fire", "is_fire"),
    ("is_fall", "is_fall"),
    ("is_drowning", "is_drowning"),
    ("is_explosion", "is_explosion"),
    ("no_knockback", "pushes_nothing"),
];

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed={DAMAGE_TYPES}");
    println!("cargo:rerun-if-changed={DAMAGE_TAGS}");
    println!("cargo:rerun-if-changed={REGISTRY_PACKETS}");

    // Read from the pack rather than from a dump beside it: a dump goes stale the moment the
    // version moves, and this one had been two types behind for a while.
    let mut damage_type_names: Vec<String> = fs::read_dir(DAMAGE_TYPES)
        .expect("the damage types the packs define")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension()? == "json").then(|| path.file_stem()?.to_str().map(str::to_string))?
        })
        .collect();
    damage_type_names.sort();

    let membership = ASKED_ABOUT
        .iter()
        .map(|(tag, asking)| {
            let asking = format_ident!("{asking}");
            let inside = read_tag(tag);
            let arms = damage_type_names
                .iter()
                .filter(|name| inside.contains(name))
                .map(|name| {
                    let name = format_ident!("{}", name.to_pascal_case());
                    quote! { Self::#name }
                })
                .collect::<Vec<_>>();
            // A tag with nothing in it is a real answer, not a mistake: vanilla keeps
            // `bypasses_cooldown` and puts nothing there.
            let answer = if arms.is_empty() {
                quote! { false }
            } else {
                quote! { matches!(self, #(#arms)|*) }
            };
            let doc = format!(" Whether this kind is in the packs' `{tag}` group.");
            quote! {
                #[doc = #doc]
                #[must_use]
                pub const fn #asking(self) -> bool {
                    #answer
                }
            }
        })
        .collect::<TokenStream>();

    // What each kind carries besides its name: the key its death message is written from, how far
    // it moves with the difficulty, and what it costs in hunger. All three are fields of the same
    // files the names came from, so reading them here costs nothing and cannot go stale.
    let details = details(&damage_type_names);

    // The number a client reads is a place in its own registry, and the registry grows: a kind
    // added in the middle shifts every kind after it. Four of the ten supported versions carry a
    // different count, so the number is looked up per version rather than assumed.
    let wire_ids = wire_ids(&damage_type_names);

    let variants = crate::array_to_tokenstream(&damage_type_names);

    let type_from_name = &damage_type_names
        .iter()
        .map(|damage_type| {
            let id = &damage_type;
            let name = format_ident!("{}", damage_type.to_pascal_case());

            quote! {
                #id => Some(Self::#name),
            }
        })
        .collect::<TokenStream>();

    let type_to_name = &damage_type_names
        .iter()
        .map(|damage_type| {
            let id = &damage_type;
            let name = format_ident!("{}", damage_type.to_pascal_case());

            quote! {
                Self::#name => #id,
            }
        })
        .collect::<TokenStream>();

    quote! {
        /// When the difficulty moves how hard a kind of damage hits.
        ///
        /// Only a mob's blow is softened on easy and sharpened on hard; a player's blow and the
        /// world's own hazards land the same whatever the difficulty.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Scaling {
            /// Never, whatever the difficulty.
            Never,
            /// Only when something living that is not a player is behind it.
            WhenCausedByLivingNonPlayer,
            /// Always.
            Always,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum DamageType {
            #variants
        }

        impl DamageType {
            #[doc = r" Try to parse a `DamageType` from a resource location string."]
            pub fn from_name(name: &str) -> Option<Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name {
                    #type_from_name
                    _ => None
                }
            }

            pub const fn to_name(&self) -> &'static str {
                match self {
                    #type_to_name
                }
            }

            #details

            #membership

            /// The number a client speaking `version` reads this kind as, if it knows it at all.
            ///
            /// [`None`] means the kind was added after that version, which is the honest answer:
            /// there is no number to send.
            #[must_use]
            pub const fn wire_id(self, version: ferrumc_net_codec::version::ProtocolVersion) -> Option<i32> {
                match DAMAGE_TYPE_IDS[version.index()][self as usize] {
                    -1 => None,
                    id => Some(id),
                }
            }
        }

        #wire_ids
    }
}

/// The place each kind sits in each version's registry, in the order of `ProtocolVersion::ALL`.
///
/// Read from the payload actually sent to the client, so the two cannot drift apart: they are the
/// same file.
fn wire_ids(names: &[String]) -> TokenStream {
    let rows = REGISTRY_PAYLOADS.iter().map(|payload| {
        let registries: indexmap::IndexMap<String, indexmap::IndexMap<String, serde_json::Value>> =
            serde_json::from_str(
                &fs::read_to_string(format!("{REGISTRY_PACKETS}/{payload}.json"))
                    .expect("a registry payload for every supported version"),
            )
            .expect("a registry payload is valid json");
        let known = registries
            .iter()
            .find(|(name, _)| name.contains("damage_type"))
            .map(|(_, entries)| entries.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();

        let ids = names.iter().map(|name| {
            let id = known
                .iter()
                .position(|known| known.strip_prefix("minecraft:").unwrap_or(known) == name)
                .map_or(-1i32, |place| {
                    i32::try_from(place).expect("a registry is not that large")
                });
            quote! { #id }
        });
        quote! { [#(#ids),*] }
    });
    let versions = REGISTRY_PAYLOADS.len();
    let count = names.len();
    quote! {
        /// Where each kind sits in each supported version's damage type registry, or -1 where the
        /// version does not have it.
        const DAMAGE_TYPE_IDS: [[i32; #count]; #versions] = [#(#rows),*];
    }
}

/// What each kind carries besides its name.
fn details(names: &[String]) -> TokenStream {
    let mut messages = TokenStream::new();
    let mut scalings = TokenStream::new();
    let mut exhaustions = TokenStream::new();

    for name in names {
        let variant = format_ident!("{}", name.to_pascal_case());
        let definition: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(format!("{DAMAGE_TYPES}/{name}.json"))
                .expect("a definition for every kind named"),
        )
        .expect("a definition is valid json");

        let message = definition["message_id"]
            .as_str()
            .expect("every kind has a message id");
        messages.extend(quote! { Self::#variant => #message, });

        let scaling = match definition["scaling"].as_str() {
            Some("never") => quote! { Scaling::Never },
            Some("always") => quote! { Scaling::Always },
            _ => quote! { Scaling::WhenCausedByLivingNonPlayer },
        };
        scalings.extend(quote! { Self::#variant => #scaling, });

        let exhaustion = definition["exhaustion"].as_f64().unwrap_or(0.0) as f32;
        exhaustions.extend(quote! { Self::#variant => #exhaustion, });
    }

    quote! {
        /// The name a death message is written from, which is not the name of the kind: falling is
        /// `fall`, burning is `onFire`, and being hit by a player is `player`.
        #[must_use]
        pub const fn message_id(self) -> &'static str {
            match self {
                #messages
            }
        }

        /// Whether the difficulty moves this kind of damage, and when.
        #[must_use]
        pub const fn scaling(self) -> Scaling {
            match self {
                #scalings
            }
        }

        /// What taking this costs a player in hunger.
        #[must_use]
        pub const fn exhaustion(self) -> f32 {
            match self {
                #exhaustions
            }
        }
    }
}

/// Which kinds of damage a tag holds. A tag naming another tag is followed, since the packs write
/// them that way.
fn read_tag(tag: &str) -> BTreeMap<String, ()> {
    let mut inside = BTreeMap::new();
    let path = format!("{DAMAGE_TAGS}/{tag}.json");
    let Ok(text) = fs::read_to_string(&path) else {
        // Vanilla keeps `bypasses_cooldown` as a tag with nothing in it. Every other name here is
        // a file that exists, and a missing one means the list has drifted from the packs.
        assert_eq!(
            tag, "bypasses_cooldown",
            "no such damage type tag in the packs: {tag}"
        );
        return inside;
    };
    let value: serde_json::Value = serde_json::from_str(&text).expect("a tag is valid json");
    let Some(values) = value.get("values").and_then(serde_json::Value::as_array) else {
        return inside;
    };
    for entry in values {
        let Some(name) = entry.as_str() else { continue };
        if let Some(other) = name.strip_prefix('#') {
            let other = other.strip_prefix("minecraft:").unwrap_or(other);
            inside.extend(read_tag(other));
        } else {
            inside.insert(
                name.strip_prefix("minecraft:").unwrap_or(name).to_string(),
                (),
            );
        }
    }
    inside
}

/// Whether a name is in a tag.
trait Holds {
    fn contains(&self, name: &str) -> bool;
}

impl Holds for BTreeMap<String, ()> {
    fn contains(&self, name: &str) -> bool {
        self.contains_key(name)
    }
}
