//! The numbers a worldgen definition is written with.
//!
//! Vanilla has three families that look alike and are not: an `IntProvider` gives a whole number,
//! a `FloatProvider` a real one, and a `HeightProvider` a height that may be written relative to
//! the world's floor or ceiling. They share type names — a `uniform` is all three — and differ in
//! their fields, so each is read where one is expected rather than guessed at.

use serde_json::Value;

/// A height, written outright or relative to the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl VerticalAnchor {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let field = |name: &str| object.get(name).and_then(Value::as_i64).map(|y| y as i32);
        field("absolute")
            .map(Self::Absolute)
            .or_else(|| field("above_bottom").map(Self::AboveBottom))
            .or_else(|| field("below_top").map(Self::BelowTop))
    }

    /// The height this means in a world running from `bottom` to `top`.
    #[must_use]
    pub fn resolve(self, bottom: i32, top: i32) -> i32 {
        match self {
            Self::Absolute(y) => y,
            Self::AboveBottom(offset) => bottom + offset,
            Self::BelowTop(offset) => top - offset,
        }
    }
}

/// A whole number, or a way of drawing one.
#[derive(Clone, Debug)]
pub enum IntProvider {
    Constant(i32),
    Uniform {
        min: i32,
        max: i32,
    },
    /// Drawn twice and the lower kept, which crowds the results towards the bottom.
    BiasedToBottom {
        min: i32,
        max: i32,
    },
    Clamped {
        source: Box<IntProvider>,
        min: i32,
        max: i32,
    },
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min: i32,
        max: i32,
    },
    /// Flat in the middle and sloping at the ends.
    Trapezoid {
        min: i32,
        max: i32,
        plateau: i32,
    },
    /// One of several, each as likely as its weight.
    WeightedList(Vec<(IntProvider, i32)>),
}

impl IntProvider {
    pub fn parse(value: &Value) -> Option<Self> {
        if let Some(constant) = value.as_i64() {
            return Some(Self::Constant(constant as i32));
        }
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let int = |name: &str| object.get(name).and_then(Value::as_i64).map(|v| v as i32);
        let float = |name: &str| object.get(name).and_then(Value::as_f64).map(|v| v as f32);
        let (min, max) = (int("min_inclusive"), int("max_inclusive"));
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "constant" => Self::Constant(int("value")?),
            "uniform" => Self::Uniform {
                min: min?,
                max: max?,
            },
            "biased_to_bottom" => Self::BiasedToBottom {
                min: min?,
                max: max?,
            },
            "clamped" => Self::Clamped {
                source: Box::new(Self::parse(object.get("source")?)?),
                min: min?,
                max: max?,
            },
            "clamped_normal" => Self::ClampedNormal {
                mean: float("mean")?,
                deviation: float("deviation")?,
                min: min?,
                max: max?,
            },
            // A whole-number trapezoid names its ends plainly, where the height one spells them
            // out. They are otherwise the same shape.
            "trapezoid" => Self::Trapezoid {
                min: int("min")?,
                max: int("max")?,
                plateau: int("plateau").unwrap_or_default(),
            },
            "weighted_list" => {
                Self::WeightedList(weighted(object.get("distribution")?, Self::parse)?)
            }
            _ => return None,
        })
    }
}

/// A real number, or a way of drawing one.
#[derive(Clone, Debug)]
pub enum FloatProvider {
    Constant(f32),
    /// Note the upper end is not included, unlike the whole-number kind.
    Uniform {
        min: f32,
        max: f32,
    },
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min: f32,
        max: f32,
    },
    Trapezoid {
        min: f32,
        max: f32,
        plateau: f32,
    },
}

impl FloatProvider {
    pub fn parse(value: &Value) -> Option<Self> {
        if let Some(constant) = value.as_f64() {
            return Some(Self::Constant(constant as f32));
        }
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let float = |name: &str| object.get(name).and_then(Value::as_f64).map(|v| v as f32);
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "constant" => Self::Constant(float("value")?),
            "uniform" => Self::Uniform {
                min: float("min_inclusive")?,
                max: float("max_exclusive")?,
            },
            "clamped_normal" => Self::ClampedNormal {
                mean: float("mean")?,
                deviation: float("deviation")?,
                min: float("min")?,
                max: float("max")?,
            },
            "trapezoid" => Self::Trapezoid {
                min: float("min")?,
                max: float("max")?,
                plateau: float("plateau")?,
            },
            _ => return None,
        })
    }
}

/// A height, or a way of drawing one.
#[derive(Clone, Debug)]
pub enum HeightProvider {
    Constant(VerticalAnchor),
    Uniform {
        min: VerticalAnchor,
        max: VerticalAnchor,
    },
    BiasedToBottom {
        min: VerticalAnchor,
        max: VerticalAnchor,
        inner: i32,
    },
    VeryBiasedToBottom {
        min: VerticalAnchor,
        max: VerticalAnchor,
        inner: i32,
    },
    Trapezoid {
        min: VerticalAnchor,
        max: VerticalAnchor,
        plateau: i32,
    },
    WeightedList(Vec<(HeightProvider, i32)>),
}

impl HeightProvider {
    pub fn parse(value: &Value) -> Option<Self> {
        // A bare anchor is a constant height, which is how most of the data writes one.
        if let Some(anchor) = VerticalAnchor::parse(value) {
            return Some(Self::Constant(anchor));
        }
        let object = value.as_object()?;
        let kind = object.get("type")?.as_str()?;
        let anchor = |name: &str| object.get(name).and_then(VerticalAnchor::parse);
        let inner = || {
            object
                .get("inner")
                .and_then(Value::as_i64)
                .map(|v| v as i32)
                .unwrap_or(1)
        };
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "constant" => Self::Constant(anchor("value")?),
            "uniform" => Self::Uniform {
                min: anchor("min_inclusive")?,
                max: anchor("max_inclusive")?,
            },
            "biased_to_bottom" => Self::BiasedToBottom {
                min: anchor("min_inclusive")?,
                max: anchor("max_inclusive")?,
                inner: inner(),
            },
            "very_biased_to_bottom" => Self::VeryBiasedToBottom {
                min: anchor("min_inclusive")?,
                max: anchor("max_inclusive")?,
                inner: inner(),
            },
            "trapezoid" => Self::Trapezoid {
                min: anchor("min_inclusive")?,
                max: anchor("max_inclusive")?,
                plateau: object
                    .get("plateau")
                    .and_then(Value::as_i64)
                    .map(|v| v as i32)
                    .unwrap_or_default(),
            },
            "weighted_list" => {
                Self::WeightedList(weighted(object.get("distribution")?, Self::parse)?)
            }
            _ => return None,
        })
    }
}

/// A `distribution` list: each entry a `data` and the `weight` it carries.
pub(crate) fn weighted<T>(
    value: &Value,
    mut parse: impl FnMut(&Value) -> Option<T>,
) -> Option<Vec<(T, i32)>> {
    value
        .as_array()?
        .iter()
        .map(|entry| {
            let weight = entry.get("weight")?.as_i64()? as i32;
            Some((parse(entry.get("data")?)?, weight))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_anchor_is_read_relative_to_the_world() {
        let absolute =
            VerticalAnchor::parse(&serde_json::json!({"absolute": 10})).expect("a valid anchor");
        assert_eq!(absolute.resolve(-64, 320), 10);

        let above =
            VerticalAnchor::parse(&serde_json::json!({"above_bottom": 8})).expect("a valid anchor");
        assert_eq!(above.resolve(-64, 320), -56);

        let below =
            VerticalAnchor::parse(&serde_json::json!({"below_top": 8})).expect("a valid anchor");
        assert_eq!(below.resolve(-64, 320), 312);
    }

    /// A bare number is a constant, which is how most counts are written.
    #[test]
    fn a_bare_number_is_a_constant() {
        assert!(matches!(
            IntProvider::parse(&serde_json::json!(4)),
            Some(IntProvider::Constant(4))
        ));
        assert!(matches!(
            FloatProvider::parse(&serde_json::json!(0.5)),
            Some(FloatProvider::Constant(f)) if (f - 0.5).abs() < 1e-6
        ));
    }

    /// The three families share type names and differ in their fields, so each is read where it is
    /// expected: a uniform whole number takes both ends, a uniform real one does not.
    #[test]
    fn the_three_families_are_told_apart_by_where_they_sit() {
        let whole = serde_json::json!({"type": "minecraft:uniform", "min_inclusive": 1, "max_inclusive": 3});
        assert!(matches!(
            IntProvider::parse(&whole),
            Some(IntProvider::Uniform { min: 1, max: 3 })
        ));
        // The same json is not a real-number provider, which wants an exclusive upper end.
        assert!(FloatProvider::parse(&whole).is_none());

        let real = serde_json::json!({"type": "minecraft:uniform", "min_inclusive": 0.0, "max_exclusive": 1.0});
        assert!(matches!(
            FloatProvider::parse(&real),
            Some(FloatProvider::Uniform { .. })
        ));

        let height = serde_json::json!({
            "type": "minecraft:uniform",
            "min_inclusive": {"above_bottom": 0},
            "max_inclusive": {"below_top": 0}
        });
        assert!(matches!(
            HeightProvider::parse(&height),
            Some(HeightProvider::Uniform { .. })
        ));
    }

    #[test]
    fn a_weighted_list_carries_its_weights() {
        let list = serde_json::json!({
            "type": "minecraft:weighted_list",
            "distribution": [{"data": 1, "weight": 3}, {"data": 2, "weight": 1}]
        });
        let Some(IntProvider::WeightedList(entries)) = IntProvider::parse(&list) else {
            panic!("a weighted list")
        };
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].1, 3);
    }
}
