use proc_macro2::TokenStream;
use quote::quote;
use std::collections::BTreeMap;
use std::fs;
use syn::{LitFloat, LitInt};

/// Which attributes each kind of entity starts with, asked of the game by
/// `scripts/extract_default_attributes.py`.
const DEFAULTS: &str = "../../../assets/extracted/default_attributes.json";

/// Where the numbers each attribute and each kind of entity travel as come from.
const REGISTRIES: &str = "../../../assets/data/registries.json";

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed={DEFAULTS}");
    println!("cargo:rerun-if-changed={REGISTRIES}");

    let defaults: BTreeMap<String, BTreeMap<String, f64>> = serde_json::from_str(
        &fs::read_to_string(DEFAULTS).expect("what each kind of entity starts with"),
    )
    .expect("the defaults are valid json");

    let registries: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(REGISTRIES).expect("the registries"))
            .expect("the registries are valid json");
    let number_of = |registry: &str, name: &str| -> Option<u16> {
        registries[registry]["entries"][format!("minecraft:{name}")]["protocol_id"]
            .as_u64()
            .and_then(|id| u16::try_from(id).ok())
    };

    // Keyed by the number the kind travels as rather than by its name, so looking one up at spawn
    // is a jump table rather than a string comparison. The entity type enum lives in another crate
    // and this one has no business knowing about it.
    let arms = defaults
        .iter()
        .filter_map(|(kind, attributes)| {
            let kind_id = number_of("minecraft:entity_type", kind)?;
            let kind_lit = LitInt::new(&kind_id.to_string(), proc_macro2::Span::call_site());

            let pairs = attributes.iter().filter_map(|(attribute, value)| {
                let id = number_of("minecraft:attribute", attribute)?;
                let id = LitInt::new(&id.to_string(), proc_macro2::Span::call_site());
                let value = number(*value);
                Some(quote! { (#id, #value) })
            });
            Some(quote! {
                #kind_lit => &[#(#pairs),*],
            })
        })
        .collect::<TokenStream>();

    quote! {
        /// Which attributes a kind of entity starts with, and what it starts them at.
        ///
        /// Keyed by the number the kind travels as. What a zombie's health and speed are is built
        /// in code by the entity class rather than written in any data file, so it is asked of the
        /// game and written out here.
        ///
        /// Nothing that does not live has any: an arrow, a boat and a dropped item all answer with
        /// nothing at all.
        ///
        /// The arms are in the order of the kinds' names, and each pair is an attribute's own
        /// number and what the kind starts it at.
        #[must_use]
        pub const fn defaults_for(entity_type: u16) -> &'static [(u16, f64)] {
            match entity_type {
                #arms
                _ => &[],
            }
        }
    }
}

/// A number written so nothing is lost, and so it is still a float literal.
fn number(value: f64) -> LitFloat {
    let written = format!("{value:?}");
    let written = if written.contains(['.', 'e', 'E']) {
        written
    } else {
        format!("{written}.0")
    };
    LitFloat::new(&written, proc_macro2::Span::call_site())
}
