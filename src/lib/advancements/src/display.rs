//! How an advancement looks on the screen, which is the half of it the client is told about.

use ferrumc_datapack::Identifier;
use ferrumc_text::TextComponent;
use serde_json::Value;

/// What shape the advancement's frame is, which also decides its colour and its announcement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Frame {
    #[default]
    Task,
    Challenge,
    Goal,
}

impl Frame {
    #[must_use]
    pub fn parse(name: &str) -> Self {
        match name {
            "challenge" => Self::Challenge,
            "goal" => Self::Goal,
            _ => Self::Task,
        }
    }

    /// The order the client reads them in.
    #[must_use]
    pub fn index(self) -> i32 {
        match self {
            Self::Task => 0,
            Self::Challenge => 1,
            Self::Goal => 2,
        }
    }
}

/// An advancement's entry on the screen.
#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub title: TextComponent,
    pub description: TextComponent,
    /// The item shown as its icon, by registry id.
    pub icon: i32,
    pub frame: Frame,
    /// The texture behind a root's tab, which only a root has.
    pub background: Option<Identifier>,
    pub show_toast: bool,
    pub announce_to_chat: bool,
    pub hidden: bool,
    pub x: f32,
    pub y: f32,
}

impl DisplayInfo {
    pub fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let component = |name: &str| {
            object
                .get(name)
                .map(ferrumc_text::from_json)
                .unwrap_or_default()
        };
        let flag = |name: &str, default: bool| {
            object.get(name).and_then(Value::as_bool).unwrap_or(default)
        };
        Some(Self {
            title: component("title"),
            description: component("description"),
            icon: object
                .get("icon")
                .and_then(|icon| icon.get("id"))
                .and_then(Value::as_str)
                .and_then(ferrumc_registry::lookup_item_protocol_id)?,
            frame: object
                .get("frame")
                .and_then(Value::as_str)
                .map(Frame::parse)
                .unwrap_or_default(),
            background: object
                .get("background")
                .and_then(Value::as_str)
                .and_then(|id| Identifier::parse(id).ok()),
            show_toast: flag("show_toast", true),
            announce_to_chat: flag("announce_to_chat", true),
            hidden: flag("hidden", false),
            // Where it sits on the tree is worked out by the server in vanilla and sent; nothing
            // lays a tree out here yet, so both are nought until something does.
            x: 0.0,
            y: 0.0,
        })
    }
}
