use heck::ToPascalCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::fs;
use syn::LitStr;

/// Where the numbers each component type travels as come from.
const REGISTRIES: &str = "../../../assets/data/registries.json";

/// Each supported version's own registry, which is where its own numbers come from.
const EXTRACTED: &str = "../../../assets/extracted";

/// The supported versions, in the order of `ProtocolVersion::ALL`.
const VERSIONS: [&str; 10] = [
    "1.21", "1.21.2", "1.21.4", "1.21.5", "1.21.6", "1.21.8", "1.21.9", "1.21.11", "26.1", "26.2",
];

/// A component whose name carries a slash — `cat/sound_variant` and its kin — turned into
/// something that can be an identifier.
fn variant_of(name: &str) -> String {
    name.replace('/', "_").to_pascal_case()
}

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed={REGISTRIES}");
    println!("cargo:rerun-if-changed={EXTRACTED}");

    let registries: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(REGISTRIES).expect("the registries"))
            .expect("the registries are valid json");
    let entries = registries["minecraft:data_component_type"]["entries"]
        .as_object()
        .expect("the registry names every component type");

    // In the registry's own order, which is what the numbers count.
    let mut names: Vec<(String, u16)> = entries
        .iter()
        .filter_map(|(name, entry)| {
            let id = u16::try_from(entry["protocol_id"].as_u64()?).ok()?;
            Some((
                name.strip_prefix("minecraft:").unwrap_or(name).to_string(),
                id,
            ))
        })
        .collect();
    names.sort_by_key(|(_, id)| *id);

    let variants = names
        .iter()
        .map(|(name, _)| {
            let variant = format_ident!("{}", variant_of(name));
            let doc = format!(" `minecraft:{name}`");
            quote! {
                #[doc = #doc]
                #variant,
            }
        })
        .collect::<TokenStream>();

    let from_name = names
        .iter()
        .map(|(name, _)| {
            let variant = format_ident!("{}", variant_of(name));
            let name = LitStr::new(name, Span::call_site());
            quote! { #name => Some(Self::#variant), }
        })
        .collect::<TokenStream>();

    let to_name = names
        .iter()
        .map(|(name, _)| {
            let variant = format_ident!("{}", variant_of(name));
            let name = LitStr::new(name, Span::call_site());
            quote! { Self::#variant => #name, }
        })
        .collect::<TokenStream>();

    // A component's number is a place in the reader's own registry, and that registry has grown
    // from 57 to 111 across the supported versions. A number meant for one version names a
    // different component in another.
    let rows = VERSIONS.iter().map(|version| {
        let theirs: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(format!("{EXTRACTED}/{version}/reports/registries.json"))
                .unwrap_or_else(|_| panic!("no registry report for {version}")),
        )
        .expect("a registry report is valid json");
        let theirs = theirs["minecraft:data_component_type"]["entries"]
            .as_object()
            .cloned()
            .unwrap_or_default();

        let ids = names.iter().map(|(name, _)| {
            let id = theirs
                .get(&format!("minecraft:{name}"))
                .and_then(|entry| entry["protocol_id"].as_i64())
                .and_then(|id| i32::try_from(id).ok())
                .unwrap_or(-1);
            quote! { #id }
        });
        quote! { [#(#ids),*] }
    });

    let all = names.iter().map(|(name, _)| {
        let variant = format_ident!("{}", variant_of(name));
        quote! { Self::#variant }
    });

    let count = names.len();
    let versions = VERSIONS.len();

    quote! {
        /// Every kind of thing an item stack can carry beyond its name and its count.
        ///
        /// Custom name, damage, enchantments, what it is worth in a fight, what it does when eaten:
        /// modern item identity is the type plus a map of these. The variants are in the order of
        /// this server's own registry.
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            bitcode_derive::Encode, bitcode_derive::Decode,
        )]
        pub enum ComponentType {
            #variants
        }

        impl ComponentType {
            /// Every kind there is, in the registry's own order.
            pub const ALL: [Self; #count] = [#(#all),*];

            /// The number it travels as, in this server's own version.
            #[must_use]
            pub const fn id(self) -> u16 {
                self as u16
            }

            /// Try to read one from a resource location.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name {
                    #from_name
                    _ => None,
                }
            }

            /// What it is called, without the namespace.
            #[must_use]
            pub const fn to_name(self) -> &'static str {
                match self {
                    #to_name
                }
            }

            /// The number a client speaking `version` reads this as, if it knows it at all.
            ///
            /// [`None`] means the version has no such component. Sending one anyway would name
            /// whatever now sits at that number, and since a component carries no length the rest
            /// of the stack would be read as nonsense.
            #[must_use]
            pub const fn wire_id(
                self,
                version: ferrumc_net_codec::version::ProtocolVersion,
            ) -> Option<u16> {
                match COMPONENT_IDS[version.index()][self as usize] {
                    -1 => None,
                    id => Some(id as u16),
                }
            }

            /// Which kind a client speaking `version` means by a number.
            #[must_use]
            pub fn from_wire_id(
                id: u16,
                version: ferrumc_net_codec::version::ProtocolVersion,
            ) -> Option<Self> {
                let theirs = &COMPONENT_IDS[version.index()];
                let at = theirs.iter().position(|known| *known == i32::from(id))?;
                Self::ALL.get(at).copied()
            }
        }

        /// Where each kind sits in each supported version's registry, or -1 where the version does
        /// not have it. Read from each version's own report.
        const COMPONENT_IDS: [[i32; #count]; #versions] = [#(#rows),*];
    }
}
