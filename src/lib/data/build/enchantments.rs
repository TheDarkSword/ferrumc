use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use syn::LitInt;

#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
pub struct Enchantment {
    /// Filled in from where it sits in the registry rather than read from the file.
    #[serde(skip)]
    pub id: u16,
    pub description: Description,
    pub min_cost: Cost,
    pub max_cost: Cost,
    pub anvil_cost: u8,
    pub slots: Vec<String>,
    pub supported_items: String,
    pub weight: u8,
    pub max_level: u8,
    #[serde(default)]
    pub exclusive_set: Option<String>,
    #[serde(default)]
    pub effects: serde_json::Value,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Description {
    pub translate: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Cost {
    pub base: f32,
    pub per_level_above_first: f32,
}

/// Where the packs define what each enchantment is.
const ENCHANTMENTS: &str = "../../../assets/extracted/26.2/data/minecraft/enchantment";

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed={ENCHANTMENTS}");
    println!("cargo:rerun-if-changed={REGISTRY_PACKETS}");

    // Read from the packs rather than from a dump beside them, and numbered by where each one sits
    // in the registry the client is actually sent — which is the only numbering that means
    // anything, and the numbering `wire_id` below is indexed by.
    let order = order_in_registry(REGISTRY_PAYLOADS[REGISTRY_PAYLOADS.len() - 1]);
    let mut enchantments: BTreeMap<String, Enchantment> = BTreeMap::new();
    for entry in fs::read_dir(ENCHANTMENTS).expect("the enchantments the packs define") {
        let path = entry.expect("a readable directory entry").path();
        if path.extension().is_none_or(|kind| kind != "json") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .expect("a name")
            .to_string();
        let mut enchantment: Enchantment =
            serde_json::from_str(&fs::read_to_string(&path).expect("a readable enchantment"))
                .unwrap_or_else(|err| panic!("could not read the enchantment {name}: {err}"));
        enchantment.id = order
            .iter()
            .position(|known| *known == name)
            .and_then(|at| u16::try_from(at).ok())
            .unwrap_or_else(|| panic!("{name} is not in the registry the client is sent"));
        enchantments.insert(name, enchantment);
    }

    let mut constants = TokenStream::new();
    let mut type_from_id_arms = TokenStream::new();
    let mut type_from_name = TokenStream::new();

    for (name, enchantment) in enchantments.iter() {
        let const_ident = format_ident!("{}", name.to_shouty_snake_case());
        let id_lit = LitInt::new(&enchantment.id.to_string(), Span::call_site());

        let effects = crate::enchantment_effects::effects_of(&enchantment.effects);
        let translate = &enchantment.description.translate;
        let min_cost_base = crate::number_f32(enchantment.min_cost.base);
        let min_cost_per_level = crate::number_f32(enchantment.min_cost.per_level_above_first);
        let max_cost_base = crate::number_f32(enchantment.max_cost.base);
        let max_cost_per_level = crate::number_f32(enchantment.max_cost.per_level_above_first);
        let anvil_cost = LitInt::new(&enchantment.anvil_cost.to_string(), Span::call_site());
        let weight = LitInt::new(&enchantment.weight.to_string(), Span::call_site());
        let max_level = LitInt::new(&enchantment.max_level.to_string(), Span::call_site());

        let supported_items = &enchantment.supported_items;

        let slots = enchantment
            .slots
            .iter()
            .map(|slot| {
                let slot_str = format_ident!("{}", slot.to_uppercase());
                quote! { EnchantmentSlot::#slot_str }
            })
            .collect::<Vec<_>>();

        let exclusive_set = match &enchantment.exclusive_set {
            Some(set) => {
                quote! { Some(#set) }
            }
            None => quote! { None },
        };

        constants.extend(quote! {
            pub const #const_ident: Enchantment = Enchantment {
                id: #id_lit,
                name: #name,
                description: #translate,
                min_cost: Cost {
                    base: #min_cost_base,
                    per_level_above_first: #min_cost_per_level,
                },
                max_cost: Cost {
                    base: #max_cost_base,
                    per_level_above_first: #max_cost_per_level,
                },
                anvil_cost: #anvil_cost,
                slots: &[#(#slots),*],
                supported_items: #supported_items,
                weight: #weight,
                max_level: #max_level,
                exclusive_set: #exclusive_set,
                effects: #effects,
            };
        });

        type_from_id_arms.extend(quote! {
            #id_lit => Some(&Self::#const_ident),
        });

        type_from_name.extend(quote! {
            #name => Some(&Self::#const_ident),
        });
    }

    // An enchantment's number is a place in the reader's own registry, and `lunge` was added in
    // 26.1 in the middle of the alphabet — which moved twenty-one of the forty-two after it.
    let wire_ids = wire_ids(&order);
    let types = crate::enchantment_effects::types();

    quote! {
        #types

        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Enchantment {
            pub id: u16,
            pub name: &'static str,
            pub description: &'static str,
            pub min_cost: Cost,
            pub max_cost: Cost,
            pub anvil_cost: u8,
            pub slots: &'static [EnchantmentSlot],
            pub supported_items: &'static str,
            pub weight: u8,
            pub max_level: u8,
            /// What it actually does, as far as this server reads.
            pub effects: &'static [Effect],
            pub exclusive_set: Option<&'static str>,
        }

        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Cost {
            pub base: f32,
            pub per_level_above_first: f32,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum EnchantmentSlot {
            MAINHAND,
            OFFHAND,
            HEAD,
            CHEST,
            LEGS,
            FEET,
            ARMOR,
            ANY,
            HAND,
        }

        impl Enchantment {
            #constants

            #[doc = r" Try to parse an `Enchantment` from a resource location string."]
            pub fn from_name(name: &str) -> Option<&'static Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name {
                    #type_from_name
                    _ => None
                }
            }

            #[doc = r" Try to get an `Enchantment` from its ID."]
            pub const fn from_id(id: u16) -> Option<&'static Self> {
                match id {
                    #type_from_id_arms
                    _ => None
                }
            }

            #[doc = r" Calculate the minimum cost for this enchantment at the given level."]
            pub const fn min_cost(&self, level: u8) -> f32 {
                self.min_cost.base + self.min_cost.per_level_above_first * (level - 1) as f32
            }

            #[doc = r" Calculate the maximum cost for this enchantment at the given level."]
            pub const fn max_cost(&self, level: u8) -> f32 {
                self.max_cost.base + self.max_cost.per_level_above_first * (level - 1) as f32
            }

            /// The number a client speaking `version` reads this as, if it knows it at all.
            ///
            /// [`None`] means the enchantment was added after that version.
            #[must_use]
            pub const fn wire_id(
                &self,
                version: ferrumc_net_codec::version::ProtocolVersion,
            ) -> Option<u16> {
                match ENCHANTMENT_IDS[version.index()][self.id as usize] {
                    -1 => None,
                    id => Some(id as u16),
                }
            }

            /// Which enchantment a client speaking `version` means by a number.
            #[must_use]
            pub fn from_wire_id(
                id: u16,
                version: ferrumc_net_codec::version::ProtocolVersion,
            ) -> Option<&'static Self> {
                let theirs = &ENCHANTMENT_IDS[version.index()];
                let at = theirs.iter().position(|known| *known == i32::from(id))?;
                Self::from_id(u16::try_from(at).ok()?)
            }
        }

        #wire_ids
    }
}

/// Where the payload each version's client is actually sent lives.
const REGISTRY_PACKETS: &str = "../../../assets/data/registry_packets";

/// The supported versions, in the order of `ProtocolVersion::ALL`.
const REGISTRY_PAYLOADS: [&str; 10] = [
    "1.21", "1.21.2", "1.21.4", "1.21.5", "1.21.6", "1.21.8", "1.21.9", "1.21.11", "26.1", "26.2",
];

/// Where each enchantment sits in each version's registry.
///
/// Read from the payload actually sent to the client, so the two cannot drift apart: they are the
/// same file.
/// The names in one version's enchantment registry, in the order it numbers them.
fn order_in_registry(version: &str) -> Vec<String> {
    let registries: indexmap::IndexMap<String, indexmap::IndexMap<String, serde_json::Value>> =
        serde_json::from_str(
            &fs::read_to_string(format!("{REGISTRY_PACKETS}/{version}.json"))
                .expect("a registry payload for every supported version"),
        )
        .expect("a registry payload is valid json");
    registries
        .iter()
        .find(|(name, _)| name.contains("enchantment"))
        .map(|(_, entries)| {
            entries
                .keys()
                .map(|name| name.strip_prefix("minecraft:").unwrap_or(name).to_string())
                .collect()
        })
        .unwrap_or_default()
}

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
            .find(|(name, _)| name.contains("enchantment"))
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
        /// Where each enchantment sits in each supported version's registry, or -1 where the
        /// version does not have it.
        const ENCHANTMENT_IDS: [[i32; #count]; #versions] = [#(#rows),*];
    }
}
