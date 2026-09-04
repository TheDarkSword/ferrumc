//! Turning the packs' enchantment effects into something that can be asked a question.
//!
//! An enchantment's effect is a tree: a value that depends on the level, hung under a kind of hook,
//! sometimes behind a requirement. Five kinds are read here — what an enchantment adds to a blow,
//! what it takes off one, what it adds to a push, and what it does to the wearer's own numbers.
//! The rest need hooks that do not exist yet and are left out rather than guessed at.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use serde_json::Value;
use syn::{LitBool, LitStr};

/// How a number that depends on the level is worked out.
fn level_value(value: &Value) -> Option<TokenStream> {
    // A bare number is a constant, which is how the packs write the simple case.
    if let Some(flat) = value.as_f64() {
        let flat = crate::number_f32(flat as f32);
        return Some(quote! { LevelValue::Flat(#flat) });
    }

    match value.get("type")?.as_str()?.strip_prefix("minecraft:")? {
        "linear" => {
            let base = crate::number_f32(value["base"].as_f64().unwrap_or(0.0) as f32);
            let step =
                crate::number_f32(value["per_level_above_first"].as_f64().unwrap_or(0.0) as f32);
            Some(quote! { LevelValue::Linear { base: #base, per_level: #step } })
        }
        "levels_squared" => {
            let added = crate::number_f32(value["added"].as_f64().unwrap_or(0.0) as f32);
            Some(quote! { LevelValue::LevelsSquared { added: #added } })
        }
        "fraction" => {
            let over = level_value(value.get("numerator")?)?;
            let under = level_value(value.get("denominator")?)?;
            Some(quote! { LevelValue::Fraction { over: &#over, under: &#under } })
        }
        "clamped" => {
            let inner = level_value(value.get("value")?)?;
            let lowest = crate::number_f32(value["min"].as_f64().unwrap_or(f64::MIN) as f32);
            let highest = crate::number_f32(value["max"].as_f64().unwrap_or(f64::MAX) as f32);
            Some(quote! {
                LevelValue::Clamped { inner: &#inner, lowest: #lowest, highest: #highest }
            })
        }
        _ => None,
    }
}

/// When an effect applies at all.
///
/// Only the one shape that matters is read: feather falling is protection that asks whether the
/// blow was a fall, and getting that wrong would make it protect against everything.
fn requirement(value: Option<&Value>) -> TokenStream {
    let Some(value) = value else {
        return quote! { Requires::Always };
    };
    let asks = value.get("condition").and_then(Value::as_str);
    if asks != Some("minecraft:damage_source_properties") {
        // Something this does not read. Refusing is the safe answer: an effect that should be
        // conditional and is applied always is worse than one that never applies.
        return quote! { Requires::SomethingUnread };
    }
    let Some(tags) = value
        .get("predicate")
        .and_then(|predicate| predicate.get("tags"))
        .and_then(Value::as_array)
    else {
        return quote! { Requires::SomethingUnread };
    };

    let each = tags.iter().filter_map(|tag| {
        let name = tag.get("id")?.as_str()?.strip_prefix("minecraft:")?;
        let expected = tag.get("expected")?.as_bool()?;
        let name = LitStr::new(name, Span::call_site());
        let expected = LitBool::new(expected, Span::call_site());
        Some(quote! { (#name, #expected) })
    });
    quote! { Requires::DamageTags(&[#(#each),*]) }
}

/// One effect of one kind, where the shape is one this reads.
fn effect_of(kind: &str, entry: &Value) -> Option<TokenStream> {
    let requires = requirement(entry.get("requirements"));

    match kind {
        // What an enchantment adds to a blow, or takes off one, or adds to a push. All three are
        // written the same way: an operation and a value.
        "damage" | "damage_protection" | "knockback" => {
            let effect = entry.get("effect")?;
            // Only `add` is read. The packs use nothing else for these three, and a multiply read
            // as an add would be silently wrong.
            if effect.get("type")?.as_str()? != "minecraft:add" {
                return None;
            }
            let value = level_value(effect.get("value")?)?;
            let hook = match kind {
                "damage" => quote! { Hook::Damage },
                "damage_protection" => quote! { Hook::Protection },
                _ => quote! { Hook::Knockback },
            };
            Some(quote! { Effect { hook: #hook, value: &#value, requires: #requires } })
        }
        // What it does to the wearer's own numbers, which the attribute system carries.
        "attributes" => {
            let attribute = entry
                .get("attribute")?
                .as_str()?
                .strip_prefix("minecraft:")?;
            let name = entry.get("id")?.as_str()?;
            let value = level_value(entry.get("amount")?)?;
            let operation = match entry.get("operation")?.as_str()? {
                "add_multiplied_base" => quote! { Operation::AddMultipliedBase },
                "add_multiplied_total" => quote! { Operation::AddMultipliedTotal },
                _ => quote! { Operation::AddValue },
            };
            let attribute = LitStr::new(attribute, Span::call_site());
            let name = LitStr::new(name, Span::call_site());
            Some(quote! {
                Effect {
                    hook: Hook::Attribute {
                        attribute: #attribute,
                        name: #name,
                        operation: #operation,
                    },
                    value: &#value,
                    requires: #requires,
                }
            })
        }
        _ => None,
    }
}

/// Everything one enchantment does, as far as this reads.
pub(crate) fn effects_of(effects: &Value) -> TokenStream {
    let Some(effects) = effects.as_object() else {
        return quote! { &[] };
    };
    let each = effects
        .iter()
        .filter_map(|(kind, entries)| {
            let kind = kind.strip_prefix("minecraft:")?;
            Some((kind, entries.as_array()?))
        })
        .flat_map(|(kind, entries)| {
            entries
                .iter()
                .filter_map(move |entry| effect_of(kind, entry))
                .collect::<Vec<_>>()
        });
    quote! { &[#(#each),*] }
}

/// The types the generated effects are written in terms of.
pub(crate) fn types() -> TokenStream {
    quote! {
        /// A number that depends on how strong the enchantment is.
        ///
        /// Level one is one, not zero — the packs count from one and so does this, which is why
        /// `per_level_above_first` is added `level - 1` times.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum LevelValue {
            /// The same at every level.
            Flat(f32),
            /// A base, plus a step for each level past the first.
            Linear { base: f32, per_level: f32 },
            /// The level squared, times something. Efficiency, which is why it runs away.
            LevelsSquared { added: f32 },
            /// One over another, both of which depend on the level.
            Fraction {
                over: &'static LevelValue,
                under: &'static LevelValue,
            },
            /// Another, held between two ends.
            Clamped {
                inner: &'static LevelValue,
                lowest: f32,
                highest: f32,
            },
        }

        impl LevelValue {
            /// What it comes to at a level, where one means level one.
            #[must_use]
            pub fn at(&self, level: u16) -> f32 {
                let level = f32::from(level.max(1));
                match self {
                    Self::Flat(flat) => *flat,
                    Self::Linear { base, per_level } => base + per_level * (level - 1.0),
                    Self::LevelsSquared { added } => level * level + added,
                    Self::Fraction { over, under } => {
                        let under = under.at(level as u16);
                        if under == 0.0 {
                            0.0
                        } else {
                            over.at(level as u16) / under
                        }
                    }
                    Self::Clamped {
                        inner,
                        lowest,
                        highest,
                    } => inner.at(level as u16).clamp(*lowest, *highest),
                }
            }
        }

        /// What an effect changes.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum Hook {
            /// Adds to what a blow is worth. Sharpness and its kin.
            Damage,
            /// Takes off what a blow comes to. Protection and its kin.
            Protection,
            /// Adds to how hard a blow pushes.
            Knockback,
            /// Moves one of the wearer's own numbers.
            Attribute {
                attribute: &'static str,
                /// What the modifier is called, which is how it is taken off again.
                name: &'static str,
                operation: Operation,
            },
        }

        /// How a modifier changes an attribute. The same three the attribute system has.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Operation {
            AddValue,
            AddMultipliedBase,
            AddMultipliedTotal,
        }

        /// When an effect applies.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub enum Requires {
            /// Every time.
            Always,
            /// Only for a blow whose kind is, or is not, in each of these groups.
            ///
            /// Feather falling is protection that asks whether the blow was a fall.
            DamageTags(&'static [(&'static str, bool)]),
            /// Something this server does not read, so the effect never applies. Being cautious
            /// the other way would have an enchantment protect against everything.
            SomethingUnread,
        }

        /// One thing an enchantment does.
        #[derive(Debug, Clone, Copy, PartialEq)]
        pub struct Effect {
            pub hook: Hook,
            pub value: &'static LevelValue,
            pub requires: Requires,
        }
    }
}
