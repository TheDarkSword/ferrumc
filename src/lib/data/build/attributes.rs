use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use syn::LitFloat;

#[derive(Deserialize, Clone, Debug)]
pub struct Attribute {
    pub id: u16,
    pub default_value: f64,
    /// How far down and up the attribute may be moved, whatever a modifier asks for.
    pub lowest: f64,
    pub highest: f64,
    /// Whether a client is told about it.
    pub syncable: bool,
}

/// A number written so nothing is lost, and so it is still a float literal.
fn number(value: f64) -> LitFloat {
    let written = format!("{value:?}");
    let written = if written.contains(['.', 'e', 'E']) {
        written
    } else {
        format!("{written}.0")
    };
    LitFloat::new(&written, Span::call_site())
}

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed=../../../assets/extracted/attributes.json");

    let attributes: BTreeMap<String, Attribute> = serde_json::from_str(
        &fs::read_to_string("../../../assets/extracted/attributes.json").unwrap(),
    )
    .expect("Failed to parse attributes.json");

    let mut constants = TokenStream::new();
    let mut type_from_id_arms = TokenStream::new();
    let mut type_from_name = TokenStream::new();
    let mut const_idents = Vec::new();

    for (name, attribute) in attributes.iter() {
        let const_ident = format_ident!("{}", name.to_shouty_snake_case());
        const_idents.push(const_ident.clone());
        let id_lit = syn::LitInt::new(&attribute.id.to_string(), Span::call_site());
        // Written out in full. Rounding to one place turned gravity's 0.08 into 0.1 and jump
        // strength's 0.42 into 0.4, which nothing noticed because nothing reads them yet.
        let default_value_lit = number(attribute.default_value);
        let lowest_lit = number(attribute.lowest);
        let highest_lit = number(attribute.highest);
        let syncable = attribute.syncable;

        constants.extend(quote! {
            pub const #const_ident: Attribute = Attribute {
                id: #id_lit,
                name: #name,
                default_value: #default_value_lit,
                lowest: #lowest_lit,
                highest: #highest_lit,
                syncable: #syncable,
            };
        });

        type_from_id_arms.extend(quote! {
            #id_lit => Some(&Self::#const_ident),
        });

        type_from_name.extend(quote! {
            #name => Some(&Self::#const_ident),
        });
    }

    quote! {

        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Attribute {
            pub id: u16,
            pub name: &'static str,
            /// What it is worth before any modifier moves it.
            pub default_value: f64,
            /// How far down and up it may be moved, whatever a modifier asks for.
            pub lowest: f64,
            pub highest: f64,
            /// Whether a client is told about it.
            pub syncable: bool,
        }

        impl Attribute {
            /// A value held to what this attribute allows.
            #[must_use]
            pub fn clamp(&self, value: f64) -> f64 {
                value.clamp(self.lowest, self.highest)
            }
        }

        impl Attribute {
            #constants

            #[doc = r" Try to parse an `Attribute` from a resource location string."]
            pub fn from_name(name: &str) -> Option<&'static Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name {
                    #type_from_name
                    _ => None
                }
            }

            #[doc = r" Try to get an `Attribute` from its ID."]
            pub const fn from_id(id: u16) -> Option<&'static Self> {
                match id {
                    #type_from_id_arms
                    _ => None
                }
            }

            #[doc = r" Get all attributes as a slice."]
            pub fn all() -> &'static [&'static Self] {
                &[#(&Self::#const_idents),*]
            }
        }
    }
}
