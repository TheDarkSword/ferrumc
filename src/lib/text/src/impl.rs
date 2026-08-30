use crate::*;
use ferrumc_nbt::{NBTSerializable, NBTSerializeOptions};
use paste::paste;
use std::fmt;
use std::ops::Add;
use std::str::FromStr;

impl From<String> for TextComponent {
    fn from(value: String) -> Self {
        Self {
            content: TextContent::Text { text: value },
            ..Default::default()
        }
    }
}

impl From<&str> for TextComponent {
    fn from(value: &str) -> Self {
        Self {
            content: TextContent::Text { text: value.into() },
            ..Default::default()
        }
    }
}

impl<T> Add<T> for TextComponent
where
    T: Into<TextComponent>,
{
    type Output = Self;

    fn add(mut self, other: T) -> Self {
        self.extra.push(other.into());
        self
    }
}

impl<T> Add<T> for TextComponentBuilder
where
    T: Into<TextComponent>,
{
    type Output = Self;

    fn add(mut self, other: T) -> Self {
        self.extra.push(other.into());
        self
    }
}

impl FromStr for TextComponent {
    type Err = serde_json::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Self::default())
        } else {
            serde_json::from_str(s)
        }
    }
}

impl From<TextComponent> for String {
    fn from(value: TextComponent) -> String {
        serde_json::to_string(&value).unwrap()
    }
}

impl fmt::Display for TextComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(value) = serde_json::to_string(self) {
            write!(f, "{value}")
        } else {
            write!(f, "Couldn't convert to String")
        }
    }
}

impl TextComponent {
    make_setters!(
        (Color, color),
        (Font, font),
        (String, insertion),
        (ClickEvent, click_event),
        (HoverEvent, hover_event)
    );
    make_bool_setters!(bold, italic, underlined, strikethrough, obfuscated);

    pub fn serialize_nbt(&self) -> Vec<u8> {
        let mut vec = Vec::new();
        NBTSerializable::serialize(self, &mut vec, &NBTSerializeOptions::Network);
        vec
    }
}

impl Default for TextContent {
    fn default() -> Self {
        TextContent::Text {
            text: String::new(),
        }
    }
}

/// Reads the three shapes a text component takes in json: a bare string, a list whose tail is
/// appended to its head, or the component object itself.
///
/// Vanilla's `ComponentSerialization.CODEC`. The derived deserializer reads only the object, which
/// is the shape components are written in on the wire; the data files use all three.
#[must_use]
pub fn from_json(value: &serde_json::Value) -> TextComponent {
    match value {
        serde_json::Value::String(text) => TextComponent::from(text.as_str()),
        serde_json::Value::Array(parts) => {
            let mut parts = parts.iter().map(from_json);
            let mut head = parts.next().unwrap_or_default();
            head.extra.extend(parts);
            head
        }
        object => serde_json::from_value(object.clone()).unwrap_or_default(),
    }
}
