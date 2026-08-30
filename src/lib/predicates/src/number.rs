//! Numbers a predicate reads: a constant, a roll, or a sum of them.
//!
//! Vanilla's `NumberProvider`. A bare number is a constant, and an object with `min` and `max` but
//! no type is a uniform roll — both shorthands the data uses constantly.

use crate::context::LootContext;
use serde_json::Value;
use tracing::warn;

/// Where a number comes from.
#[derive(Clone, Debug)]
pub enum NumberProvider {
    Constant(f32),
    Uniform {
        min: Box<NumberProvider>,
        max: Box<NumberProvider>,
    },
    /// `n` rolls of a `p` chance, counted.
    Binomial {
        n: Box<NumberProvider>,
        p: Box<NumberProvider>,
    },
    Sum(Vec<NumberProvider>),
    /// The level of the enchantment that caused this, which the context carries.
    EnchantmentLevel,
    /// A provider whose source does not exist yet. Reads as zero, and says so once.
    Unsupported(&'static str),
}

impl NumberProvider {
    pub fn parse(value: &Value) -> Option<Self> {
        if let Some(constant) = value.as_f64() {
            return Some(Self::Constant(constant as f32));
        }
        let object = value.as_object()?;
        let kind = object.get("type").and_then(Value::as_str);
        // An object with min and max and no type is a uniform roll, which is what the data writes.
        let kind = kind.unwrap_or("minecraft:uniform");
        let field = |name: &str| object.get(name).and_then(Self::parse).map(Box::new);
        Some(match kind.strip_prefix("minecraft:").unwrap_or(kind) {
            "constant" => Self::Constant(object.get("value")?.as_f64()? as f32),
            "uniform" => Self::Uniform {
                min: field("min")?,
                max: field("max")?,
            },
            "binomial" => Self::Binomial {
                n: field("n")?,
                p: field("p")?,
            },
            "sum" => Self::Sum(
                object
                    .get("summands")?
                    .as_array()?
                    .iter()
                    .filter_map(Self::parse)
                    .collect(),
            ),
            "enchantment_level" => Self::EnchantmentLevel,
            "score" => Self::Unsupported("score"),
            "storage" => Self::Unsupported("storage"),
            "environment_attribute" => Self::Unsupported("environment_attribute"),
            _ => return None,
        })
    }

    pub fn float(&self, context: &mut LootContext) -> f32 {
        match self {
            Self::Constant(value) => *value,
            Self::Uniform { min, max } => {
                let (min, max) = (min.float(context), max.float(context));
                if min >= max {
                    min
                } else {
                    context.next_float() * (max - min) + min
                }
            }
            Self::Binomial { .. } => self.int(context) as f32,
            Self::Sum(summands) => summands
                .iter()
                .map(|summand| summand.float(context))
                .sum::<f32>(),
            Self::EnchantmentLevel => context.params.enchantment_level.unwrap_or_default() as f32,
            Self::Unsupported(kind) => {
                warn!("number provider {kind} is not supported yet, reading as zero");
                0.0
            }
        }
    }

    pub fn int(&self, context: &mut LootContext) -> i32 {
        match self {
            // A uniform roll picks a whole number between its ends inclusive rather than rounding
            // a float, so both ends come up as often as anything between them.
            Self::Uniform { min, max } => {
                let (min, max) = (min.int(context), max.int(context));
                context.next_int(min, max)
            }
            Self::Binomial { n, p } => {
                let n = n.int(context);
                let p = p.float(context);
                (0..n).filter(|_| context.next_float() < p).count() as i32
            }
            // A sum floors rather than rounds, which is what vanilla does with it.
            Self::Sum(_) => self.float(context).floor() as i32,
            other => other.float(context).round() as i32,
        }
    }
}

/// A range a number has to land in, whose ends are themselves numbers to work out.
#[derive(Clone, Debug, Default)]
pub struct IntRange {
    pub min: Option<NumberProvider>,
    pub max: Option<NumberProvider>,
}

impl IntRange {
    /// Reads a bare number, meaning exactly that, or an object with either end.
    pub fn parse(value: &Value) -> Option<Self> {
        if let Some(exact) = value.as_i64() {
            let exact = NumberProvider::Constant(exact as f32);
            return Some(Self {
                min: Some(exact.clone()),
                max: Some(exact),
            });
        }
        let object = value.as_object()?;
        Some(Self {
            min: object.get("min").and_then(NumberProvider::parse),
            max: object.get("max").and_then(NumberProvider::parse),
        })
    }

    /// Pulls a number into the range, which is what a limit does rather than a test.
    pub fn clamp(&self, context: &mut LootContext, value: i32) -> i32 {
        let mut value = value;
        if let Some(min) = &self.min {
            value = value.max(min.int(context));
        }
        if let Some(max) = &self.max {
            value = value.min(max.int(context));
        }
        value
    }

    pub fn matches(&self, context: &mut LootContext, value: i32) -> bool {
        if let Some(min) = &self.min {
            if value < min.int(context) {
                return false;
            }
        }
        if let Some(max) = &self.max {
            if value > max.int(context) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::LootParams;
    use rand::SeedableRng;

    fn with_context(f: impl FnOnce(&mut LootContext)) {
        let mut random = rand::rngs::StdRng::seed_from_u64(7);
        let mut context = LootContext::new(LootParams::default(), &mut random);
        f(&mut context);
    }

    #[test]
    fn a_bare_number_is_a_constant() {
        with_context(|context| {
            let provider = NumberProvider::parse(&serde_json::json!(3)).expect("a number");
            assert_eq!(provider.int(context), 3);
        });
    }

    #[test]
    fn min_and_max_with_no_type_is_a_uniform_roll() {
        with_context(|context| {
            let provider =
                NumberProvider::parse(&serde_json::json!({"min": 2, "max": 4})).expect("a roll");
            for _ in 0..200 {
                let rolled = provider.int(context);
                assert!((2..=4).contains(&rolled), "rolled {rolled}");
            }
        });
    }

    #[test]
    fn a_uniform_roll_reaches_both_ends() {
        with_context(|context| {
            let provider =
                NumberProvider::parse(&serde_json::json!({"min": 0, "max": 1})).expect("a roll");
            let mut seen = [false; 2];
            for _ in 0..200 {
                seen[provider.int(context) as usize] = true;
            }
            assert_eq!(seen, [true, true], "a roll of nought to one gives both");
        });
    }

    #[test]
    fn a_binomial_counts_its_successes() {
        with_context(|context| {
            let never =
                NumberProvider::parse(&serde_json::json!({"type": "binomial", "n": 8, "p": 0.0}))
                    .expect("a binomial");
            assert_eq!(never.int(context), 0);
            let always =
                NumberProvider::parse(&serde_json::json!({"type": "binomial", "n": 8, "p": 1.0}))
                    .expect("a binomial");
            assert_eq!(always.int(context), 8);
        });
    }

    #[test]
    fn a_range_holds_its_ends() {
        with_context(|context| {
            let range = IntRange::parse(&serde_json::json!({"min": 2, "max": 4})).expect("a range");
            assert!(!range.matches(context, 1));
            assert!(range.matches(context, 2));
            assert!(range.matches(context, 4));
            assert!(!range.matches(context, 5));

            let exact = IntRange::parse(&serde_json::json!(3)).expect("a range");
            assert!(exact.matches(context, 3) && !exact.matches(context, 4));
        });
    }
}
