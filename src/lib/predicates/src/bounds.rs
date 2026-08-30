//! `{"min": 2, "max": 5}`, or a bare number meaning both.
//!
//! Vanilla's `MinMaxBounds`. It has an integer flavour and a floating one; the only thing that
//! separates them is what they parse from, and every value they compare fits a `f64` exactly, so
//! there is one type here.

use serde_json::Value;

/// A closed range with either end left open.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Bounds {
    /// Accepts everything.
    pub const ANY: Self = Self {
        min: None,
        max: None,
    };

    pub fn exactly(value: f64) -> Self {
        Self {
            min: Some(value),
            max: Some(value),
        }
    }

    /// Reads a bare number, or an object with either end.
    pub fn parse(value: &Value) -> Option<Self> {
        if let Some(exact) = value.as_f64() {
            return Some(Self::exactly(exact));
        }
        let object = value.as_object()?;
        Some(Self {
            min: object.get("min").and_then(Value::as_f64),
            max: object.get("max").and_then(Value::as_f64),
        })
    }

    /// Reads the field of this name, defaulting to accepting everything.
    pub fn field(parent: &Value, name: &str) -> Self {
        parent.get(name).and_then(Self::parse).unwrap_or(Self::ANY)
    }

    #[must_use]
    pub fn is_any(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    #[must_use]
    pub fn matches(&self, value: f64) -> bool {
        self.min.is_none_or(|min| value >= min) && self.max.is_none_or(|max| value <= max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_number_is_both_ends() {
        let bounds = Bounds::parse(&serde_json::json!(3)).expect("a number is a bound");
        assert!(bounds.matches(3.0));
        assert!(!bounds.matches(2.0));
        assert!(!bounds.matches(4.0));
    }

    #[test]
    fn either_end_may_be_left_out() {
        let at_least = Bounds::parse(&serde_json::json!({"min": 2})).expect("an object is a bound");
        assert!(at_least.matches(2.0) && at_least.matches(1000.0) && !at_least.matches(1.0));

        let at_most = Bounds::parse(&serde_json::json!({"max": 2})).expect("an object is a bound");
        assert!(at_most.matches(2.0) && at_most.matches(-5.0) && !at_most.matches(3.0));

        assert!(Bounds::ANY.is_any() && Bounds::ANY.matches(f64::MIN));
    }

    #[test]
    fn a_missing_field_accepts_everything() {
        let bounds = Bounds::field(&serde_json::json!({}), "count");
        assert!(bounds.is_any());
    }
}
