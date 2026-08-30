//! The clientbound **Update Advancements** packet.
//!
//! Everything the advancement screen draws: the tree itself, where each entry sits, and how far
//! along the player is. The client keeps what it is sent, so a full set with `reset` is what a
//! player is given on joining and a partial set is what they get as they earn things.

use ferrumc_advancements::display::DisplayInfo;
use ferrumc_advancements::{Advancements, PlayerAdvancements};
use ferrumc_macros::{packet, NetEncode};
use ferrumc_nbt::NBT;
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::length_prefixed_vec::LengthPrefixedVec;
use ferrumc_net_codec::net_types::prefixed_optional::PrefixedOptional;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_text::TextComponent;

#[derive(NetEncode)]
#[packet(packet_id = "update_advancements", state = "play")]
pub struct UpdateAdvancementsPacket {
    /// Whether to forget what was sent before. True for the set a player joins with.
    pub reset: bool,
    pub added: LengthPrefixedVec<AdvancementEntry>,
    /// The ones to forget, by name.
    pub removed: LengthPrefixedVec<String>,
    pub progress: LengthPrefixedVec<ProgressEntry>,
    /// Whether the screen is offered at all.
    pub show_advancements: bool,
}

#[derive(NetEncode)]
pub struct AdvancementEntry {
    pub id: String,
    pub parent: PrefixedOptional<String>,
    pub display: PrefixedOptional<Display>,
    /// Which criteria together count as done: every group needs one of its own.
    pub requirements: LengthPrefixedVec<LengthPrefixedVec<String>>,
    pub sends_telemetry_event: bool,
}

/// An advancement's entry on the screen.
///
/// Written by hand because the background is only there when the flags say so.
pub struct Display {
    pub title: NBT<TextComponent>,
    pub description: NBT<TextComponent>,
    /// The icon, as an item template: what it is, how many, and the components it changes.
    pub icon: VarInt,
    pub frame: VarInt,
    pub background: Option<String>,
    pub show_toast: bool,
    pub hidden: bool,
    pub x: f32,
    pub y: f32,
}

impl Display {
    /// Which of the three optional halves are there, as the client reads them.
    fn flags(&self) -> i32 {
        let mut flags = 0;
        if self.background.is_some() {
            flags |= 1;
        }
        if self.show_toast {
            flags |= 2;
        }
        if self.hidden {
            flags |= 4;
        }
        flags
    }
}

impl NetEncode for Display {
    fn encode<W: std::io::Write>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        self.title.encode(writer, opts)?;
        self.description.encode(writer, opts)?;
        // An item template: the item, how many, and no component added or taken away.
        self.icon.encode(writer, opts)?;
        VarInt::new(1).encode(writer, opts)?;
        VarInt::new(0).encode(writer, opts)?;
        VarInt::new(0).encode(writer, opts)?;
        self.frame.encode(writer, opts)?;
        // A plain four-byte int rather than the varint most counts use.
        self.flags().encode(writer, opts)?;
        if let Some(background) = &self.background {
            background.encode(writer, opts)?;
        }
        self.x.encode(writer, opts)?;
        self.y.encode(writer, opts)
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer, opts)?;
        <W as tokio::io::AsyncWriteExt>::write_all(writer, &buffer).await?;
        Ok(())
    }
}

#[derive(NetEncode)]
pub struct ProgressEntry {
    pub id: String,
    pub criteria: LengthPrefixedVec<CriterionProgress>,
}

#[derive(NetEncode)]
pub struct CriterionProgress {
    pub name: String,
    /// When it was earned, in milliseconds since the epoch, or nothing where it has not been.
    pub obtained: PrefixedOptional<i64>,
}

impl UpdateAdvancementsPacket {
    /// The whole tree and everything the player has done, which is what they join with.
    #[must_use]
    pub fn everything(advancements: &Advancements, player: &PlayerAdvancements) -> Self {
        let added = advancements
            .iter()
            .map(|(id, advancement)| AdvancementEntry {
                id: id.to_owned(),
                parent: match &advancement.parent {
                    Some(parent) => PrefixedOptional::Some(parent.to_string()),
                    None => PrefixedOptional::None,
                },
                display: match &advancement.display {
                    Some(display) => {
                        PrefixedOptional::Some(entry_display(display, advancements.position(id)))
                    }
                    None => PrefixedOptional::None,
                },
                requirements: LengthPrefixedVec::new(
                    advancement
                        .requirements
                        .0
                        .iter()
                        .map(|group| LengthPrefixedVec::new(group.clone()))
                        .collect(),
                ),
                sends_telemetry_event: advancement.sends_telemetry_event,
            })
            .collect();

        Self {
            reset: true,
            added: LengthPrefixedVec::new(added),
            removed: LengthPrefixedVec::new(Vec::new()),
            progress: LengthPrefixedVec::new(progress_of(player)),
            show_advancements: true,
        }
    }

    /// Only what changed, which is what a player is sent as they earn things.
    #[must_use]
    pub fn changed(player: &PlayerAdvancements) -> Self {
        Self {
            reset: false,
            added: LengthPrefixedVec::new(Vec::new()),
            removed: LengthPrefixedVec::new(Vec::new()),
            progress: LengthPrefixedVec::new(progress_of(player)),
            show_advancements: true,
        }
    }
}

fn progress_of(player: &PlayerAdvancements) -> Vec<ProgressEntry> {
    player
        .progress
        .iter()
        .map(|(id, progress)| ProgressEntry {
            id: id.clone(),
            criteria: LengthPrefixedVec::new(
                progress
                    .criteria
                    .iter()
                    .map(|(name, obtained)| CriterionProgress {
                        name: name.clone(),
                        obtained: PrefixedOptional::Some(*obtained),
                    })
                    .collect(),
            ),
        })
        .collect()
}

fn entry_display(display: &DisplayInfo, (x, y): (f32, f32)) -> Display {
    Display {
        title: NBT::new(display.title.clone()),
        description: NBT::new(display.description.clone()),
        icon: VarInt::new(display.icon),
        frame: VarInt::new(display.frame.index()),
        background: display.background.as_ref().map(ToString::to_string),
        show_toast: display.show_toast,
        hidden: display.hidden,
        x,
        y,
    }
}
