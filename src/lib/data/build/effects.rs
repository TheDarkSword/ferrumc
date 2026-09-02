use heck::ToPascalCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::{collections::BTreeMap, fs};
use syn::{LitFloat, LitInt, LitStr};

/// What the game says a status effect is, asked of it by `scripts/extract_effects.py`.
#[derive(Deserialize)]
struct EffectData {
    id: u16,
    category: String,
    color: i32,
    /// Whether it lands all at once rather than lasting.
    instant: bool,
    /// Which of the holder's numbers it moves, and by how much for one level.
    attributes: BTreeMap<String, EffectModifier>,
}

#[derive(Deserialize)]
struct EffectModifier {
    amount: f64,
    operation: String,
    id: String,
}

pub(crate) fn build() -> TokenStream {
    println!("cargo:rerun-if-changed=../../../assets/extracted/effect.json");

    let effects: BTreeMap<String, EffectData> =
        serde_json::from_str(&fs::read_to_string("../../../assets/extracted/effect.json").unwrap())
            .expect("Failed to parse effect.json");

    let effect_names: Vec<String> = effects.keys().cloned().collect();

    let mut ids = TokenStream::new();
    let mut categories = TokenStream::new();
    let mut colours = TokenStream::new();
    let mut instant_variants = Vec::new();
    let mut modifiers = TokenStream::new();

    for (name, effect) in &effects {
        let variant = format_ident!("{}", name.to_pascal_case());

        let id = LitInt::new(&effect.id.to_string(), Span::call_site());
        ids.extend(quote! { Self::#variant => #id, });

        let category = format_ident!("{}", effect.category.to_pascal_case());
        categories.extend(quote! { Self::#variant => Category::#category, });

        let colour = LitInt::new(&effect.color.to_string(), Span::call_site());
        colours.extend(quote! { Self::#variant => #colour, });

        if effect.instant {
            instant_variants.push(quote! { Self::#variant });
        }

        let each = effect.attributes.iter().map(|(attribute, modifier)| {
            let attribute = LitStr::new(attribute, Span::call_site());
            let amount = LitFloat::new(&format!("{:?}", modifier.amount), Span::call_site());
            let operation = format_ident!("{}", modifier.operation.to_pascal_case());
            let id = LitStr::new(&modifier.id, Span::call_site());
            quote! {
                EffectModifier {
                    attribute: #attribute,
                    amount: #amount,
                    operation: Operation::#operation,
                    name: #id,
                }
            }
        });
        modifiers.extend(quote! { Self::#variant => &[#(#each),*], });
    }

    let variants = crate::array_to_tokenstream(&effect_names);

    let type_from_name = &effect_names
        .iter()
        .map(|effect| {
            let id = &effect;
            let name = format_ident!("{}", effect.to_pascal_case());

            quote! {
                #id => Some(Self::#name),
            }
        })
        .collect::<TokenStream>();

    let type_to_name = &effect_names
        .iter()
        .map(|effect| {
            let id = &effect;
            let name = format_ident!("{}", effect.to_pascal_case());

            quote! {
                Self::#name => #id,
            }
        })
        .collect::<TokenStream>();

    quote! {
        /// Whether an effect helps, hurts, or does neither. A client draws the three differently
        /// and milk takes all of them away regardless.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Category {
            Beneficial,
            Neutral,
            Harmful,
        }

        /// How a modifier changes what an attribute is worth.
        ///
        /// The same three the attribute system has; named again here so this module can be read
        /// without one, and matched across at the one place they meet.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Operation {
            AddValue,
            AddMultipliedBase,
            AddMultipliedTotal,
        }

        /// One number an effect moves, and by how much for a single level.
        ///
        /// The amount is what one level is worth; every level after it is a multiple, which is why
        /// speed II is exactly twice speed I rather than a separate modifier.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct EffectModifier {
            pub attribute: &'static str,
            pub amount: f64,
            pub operation: Operation,
            /// What the modifier is called, which is how it is taken away again.
            pub name: &'static str,
        }

        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash,
            bitcode_derive::Encode, bitcode_derive::Decode,
        )]
        pub enum Effect {
            #variants
        }

        impl Effect {
            /// The number it travels as, in this server's own version.
            #[must_use]
            pub const fn id(self) -> u16 {
                match self {
                    #ids
                }
            }

            /// Whether it helps, hurts, or does neither.
            #[must_use]
            pub const fn category(self) -> Category {
                match self {
                    #categories
                }
            }

            /// The colour a client draws it, as packed red, green and blue.
            #[must_use]
            pub const fn colour(self) -> i32 {
                match self {
                    #colours
                }
            }

            /// Whether it lands all at once rather than lasting.
            ///
            /// The three that do are healing, harming and saturation: they are applied once and
            /// never held.
            #[must_use]
            pub const fn is_instant(self) -> bool {
                matches!(self, #(#instant_variants)|*)
            }

            /// Which of the holder's numbers it moves.
            ///
            /// Twelve of the forty move anything at all; the rest are read by the code that cares
            /// about them, or by a client.
            #[must_use]
            pub const fn modifiers(self) -> &'static [EffectModifier] {
                match self {
                    #modifiers
                }
            }
            #[doc = r" Try to parse an `Effect` from a resource location string."]
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
        }
    }
}
