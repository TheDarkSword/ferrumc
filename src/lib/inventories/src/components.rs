//! What an item stack carries beyond its name and its count.
//!
//! Modern item identity is a type plus a map: a custom name, how damaged it is, what it is
//! enchanted with, what it does when eaten. A stack does not carry the whole map — it carries a
//! **patch** over what the item type already says, so a plain diamond sword sends nothing at all
//! and a named one sends only the name.
//!
//! The part that makes this delicate is that a component carries **no length**. A reader that does
//! not know what shape a component is cannot skip it: it has to read the payload or give up, since
//! everything after it would be read at the wrong offset. So an unknown component is an error here
//! rather than something quietly stepped over — a loud failure beats a stack of nonsense.

use bitcode_derive::{Decode, Encode};
use ferrumc_data::generated::components::ComponentType;
use ferrumc_data::generated::enchantments::{Enchantment, Hook, Requires};
use ferrumc_net_codec::decode::errors::NetDecodeError;
use ferrumc_net_codec::decode::{NetDecode, NetDecodeOpts};
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::version::ProtocolVersion;
use ferrumc_text::TextComponent;
use std::io::{Read, Write};

/// What shape a component's payload is on the wire.
///
/// Far fewer shapes than there are components: most of the hundred and eleven are a number, a flag
/// or a piece of NBT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Nothing follows at all, as for `unbreakable`.
    Nothing,
    /// A variable-length number, as for `damage`.
    Number,
    /// One byte, as for `enchantment_glint_override`.
    Flag,
    /// Four bytes of packed colour, as for `dyed_color`.
    Colour,
    /// A piece of text, as for `custom_name`.
    Text,
    /// A list of text, as for `lore`.
    Lines,
    /// A map of enchantment to level.
    Enchantments,
    /// A compound this server does not read into anything, kept as it arrived.
    Nbt,
}

/// What a component is worth on one stack.
#[derive(Debug, Clone, PartialEq, Encode, Decode)]
pub enum Value {
    Nothing,
    Number(i32),
    Flag(bool),
    Colour(i32),
    /// A piece of text, kept as the NBT it travels as.
    ///
    /// Kept rather than read: a custom name is written by a client and sent back to clients, and
    /// nothing here needs to look inside one. Keeping the bytes is lossless and does not need a
    /// reader for every shape a text component can take.
    Text(Vec<u8>),
    Lines(Vec<Vec<u8>>),
    /// Each entry is an enchantment as this server numbers it, and a level.
    Enchantments(Vec<(u16, u16)>),
    /// Kept as the bytes it arrived as, so it survives a round trip without being understood.
    Nbt(Vec<u8>),
}

impl Value {
    /// A piece of text, from something written here rather than read off the wire.
    #[must_use]
    pub fn text(text: &TextComponent) -> Self {
        Self::Text(as_nbt(text))
    }

    /// Several lines of it.
    #[must_use]
    pub fn lines(lines: &[TextComponent]) -> Self {
        Self::Lines(lines.iter().map(as_nbt).collect())
    }

    /// Which shape it is, which is what says how it is written.
    #[must_use]
    pub const fn shape(&self) -> Shape {
        match self {
            Self::Nothing => Shape::Nothing,
            Self::Number(_) => Shape::Number,
            Self::Flag(_) => Shape::Flag,
            Self::Colour(_) => Shape::Colour,
            Self::Text(_) => Shape::Text,
            Self::Lines(_) => Shape::Lines,
            Self::Enchantments(_) => Shape::Enchantments,
            Self::Nbt(_) => Shape::Nbt,
        }
    }
}

/// What shape a kind of component is, where this server knows.
///
/// Everything not named here is one this server cannot read, and reading one is an error rather
/// than a guess. Adding a kind is adding a line, once its shape has been read off the game.
#[must_use]
pub const fn shape_of(kind: ComponentType) -> Option<Shape> {
    Some(match kind {
        ComponentType::MaxStackSize
        | ComponentType::MaxDamage
        | ComponentType::Damage
        | ComponentType::RepairCost
        | ComponentType::Rarity
        | ComponentType::MapId
        | ComponentType::OminousBottleAmplifier => Shape::Number,

        ComponentType::Unbreakable | ComponentType::CreativeSlotLock => Shape::Nothing,

        ComponentType::EnchantmentGlintOverride => Shape::Flag,

        ComponentType::DyedColor | ComponentType::MapColor => Shape::Colour,

        ComponentType::CustomName | ComponentType::ItemName => Shape::Text,

        ComponentType::Lore => Shape::Lines,

        ComponentType::Enchantments | ComponentType::StoredEnchantments => Shape::Enchantments,

        ComponentType::CustomData | ComponentType::BlockEntityData | ComponentType::Bees => {
            Shape::Nbt
        }

        _ => return None,
    })
}

/// The patch one stack carries over what its type already says.
///
/// Kept sorted by kind so two stacks that carry the same thing compare equal however they were
/// built, which is what lets them merge.
#[derive(Debug, Clone, Default, PartialEq, Encode, Decode)]
pub struct Components {
    set: Vec<(ComponentType, Value)>,
    /// What the type has and this stack does not, which is how a component is taken away.
    removed: Vec<ComponentType>,
}

impl Components {
    /// Nothing beyond what the item type says.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            set: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// Whether it says anything at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.removed.is_empty()
    }

    /// Sets one, replacing whatever was there.
    pub fn set(&mut self, kind: ComponentType, value: Value) {
        self.removed.retain(|held| *held != kind);
        match self.set.binary_search_by_key(&kind, |(held, _)| *held) {
            Ok(at) => self.set[at].1 = value,
            Err(at) => self.set.insert(at, (kind, value)),
        }
    }

    /// Takes one away, which for a component the type provides means saying so outright.
    pub fn remove(&mut self, kind: ComponentType) {
        self.set.retain(|(held, _)| *held != kind);
        if let Err(at) = self.removed.binary_search(&kind) {
            self.removed.insert(at, kind);
        }
    }

    /// What one is worth on this stack, where the stack says.
    #[must_use]
    pub fn get(&self, kind: ComponentType) -> Option<&Value> {
        self.set
            .binary_search_by_key(&kind, |(held, _)| *held)
            .ok()
            .map(|at| &self.set[at].1)
    }

    /// Everything the stack sets.
    pub fn iter(&self) -> impl Iterator<Item = (ComponentType, &Value)> {
        self.set.iter().map(|(kind, value)| (*kind, value))
    }

    /// Everything the stack takes away.
    pub fn removed(&self) -> impl Iterator<Item = ComponentType> + '_ {
        self.removed.iter().copied()
    }

    /// A shorthand for the one people set most.
    pub fn set_name(&mut self, name: &TextComponent) {
        self.set(ComponentType::CustomName, Value::text(name));
    }

    /// How damaged it is.
    #[must_use]
    pub fn damage(&self) -> i32 {
        match self.get(ComponentType::Damage) {
            Some(Value::Number(damage)) => *damage,
            _ => 0,
        }
    }

    /// Everything it is enchanted with, and how strongly.
    pub fn enchantments(&self) -> impl Iterator<Item = (&'static Enchantment, u16)> + '_ {
        let held = match self.get(ComponentType::Enchantments) {
            Some(Value::Enchantments(held)) => held.as_slice(),
            _ => &[],
        };
        held.iter()
            .filter_map(|(id, level)| Some((Enchantment::from_id(*id)?, *level)))
    }

    /// What everything on it adds up to at one hook.
    ///
    /// An effect behind a requirement is only counted where `applies` says so — feather falling
    /// only guards against a fall, and counting it against everything would make it armour.
    #[must_use]
    pub fn adds_up_at(&self, hook: Hook, applies: impl Fn(Requires) -> bool) -> f32 {
        self.enchantments()
            .flat_map(|(enchantment, level)| {
                enchantment
                    .effects
                    .iter()
                    .map(move |effect| (effect, level))
            })
            .filter(|(effect, _)| effect.hook == hook && applies(effect.requires))
            .map(|(effect, level)| effect.value.at(level))
            .sum()
    }

    /// What level of an enchantment it carries.
    #[must_use]
    pub fn enchantment(&self, enchantment: &Enchantment) -> u16 {
        match self.get(ComponentType::Enchantments) {
            Some(Value::Enchantments(held)) => held
                .iter()
                .find(|(id, _)| *id == enchantment.id)
                .map_or(0, |(_, level)| *level),
            _ => 0,
        }
    }
}

impl NetEncode for Components {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        let nested = opts.nested();

        // Whatever the reader has never heard of is left out, and counted after the leaving out —
        // a count that does not match what follows is how a client reads the rest of a container
        // as component ids.
        let set: Vec<(u16, &Value)> = self
            .set
            .iter()
            .filter_map(|(kind, value)| Some((kind.wire_id(opts.version)?, value)))
            .collect();
        let removed: Vec<u16> = self
            .removed
            .iter()
            .filter_map(|kind| kind.wire_id(opts.version))
            .collect();

        VarInt::new(i32::try_from(set.len()).unwrap_or(i32::MAX)).encode(writer, &nested)?;
        VarInt::new(i32::try_from(removed.len()).unwrap_or(i32::MAX)).encode(writer, &nested)?;

        for (id, value) in set {
            VarInt::new(i32::from(id)).encode(writer, &nested)?;
            write_value(writer, &nested, value, opts.version)?;
        }
        for id in removed {
            VarInt::new(i32::from(id)).encode(writer, &nested)?;
        }
        Ok(())
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        let mut buffer = Vec::new();
        self.encode(&mut buffer, opts)?;
        buffer.encode_async(writer, &opts.nested()).await
    }
}

impl NetDecode for Components {
    /// Read as this server's own version numbers things.
    ///
    /// An older client's packets are brought to this shape by the translation layer before they
    /// reach here, so there is no other version to read as.
    fn decode<R: Read>(reader: &mut R, opts: &NetDecodeOpts) -> Result<Self, NetDecodeError> {
        let set_count = VarInt::decode(reader, opts)?.0.max(0);
        let removed_count = VarInt::decode(reader, opts)?.0.max(0);

        let mut components = Self::none();
        for _ in 0..set_count {
            let id = VarInt::decode(reader, opts)?.0;
            let kind = kind_of(id, ProtocolVersion::CURRENT)?;
            let shape = shape_of(kind).ok_or_else(|| unreadable(kind))?;
            let value = read_value(reader, opts, shape)?;
            components.set(kind, value);
        }
        for _ in 0..removed_count {
            let id = VarInt::decode(reader, opts)?.0;
            components.remove(kind_of(id, ProtocolVersion::CURRENT)?);
        }
        Ok(components)
    }

    async fn decode_async<R: tokio::io::AsyncRead + Unpin>(
        _reader: &mut R,
        _opts: &NetDecodeOpts,
    ) -> Result<Self, NetDecodeError> {
        Err(NetDecodeError::ExternalError(
            "components are read from a buffer rather than a stream".into(),
        ))
    }
}

/// Which kind a number names, for the version that sent it.
fn kind_of(id: i32, version: ProtocolVersion) -> Result<ComponentType, NetDecodeError> {
    u16::try_from(id)
        .ok()
        .and_then(|id| ComponentType::from_wire_id(id, version))
        .ok_or_else(|| {
            NetDecodeError::ExternalError(format!("no component {id} in {version:?}").into())
        })
}

/// A component this server has no shape for.
fn unreadable(kind: ComponentType) -> NetDecodeError {
    NetDecodeError::ExternalError(
        format!(
            "no shape known for the component {}, and one carries no length to step over",
            kind.to_name()
        )
        .into(),
    )
}

/// Writes one payload.
fn write_value<W: Write>(
    writer: &mut W,
    opts: &NetEncodeOpts,
    value: &Value,
    version: ProtocolVersion,
) -> Result<(), NetEncodeError> {
    match value {
        Value::Nothing => Ok(()),
        Value::Number(number) => VarInt::new(*number).encode(writer, opts),
        Value::Flag(flag) => flag.encode(writer, opts),
        Value::Colour(colour) => colour.encode(writer, opts),
        Value::Text(bytes) => {
            writer.write_all(bytes)?;
            Ok(())
        }
        Value::Lines(lines) => {
            VarInt::new(i32::try_from(lines.len()).unwrap_or(i32::MAX)).encode(writer, opts)?;
            for line in lines {
                writer.write_all(line)?;
            }
            Ok(())
        }
        Value::Enchantments(held) => {
            // An enchantment travels as a place in the reader's own registry, and one it has never
            // heard of is left out rather than named as another.
            let known: Vec<(u16, u16)> = held
                .iter()
                .filter_map(|(id, level)| {
                    let id = Enchantment::from_id(*id)?.wire_id(version)?;
                    Some((id, *level))
                })
                .collect();
            VarInt::new(i32::try_from(known.len()).unwrap_or(i32::MAX)).encode(writer, opts)?;
            for (id, level) in known {
                VarInt::new(i32::from(id)).encode(writer, opts)?;
                VarInt::new(i32::from(level)).encode(writer, opts)?;
            }
            Ok(())
        }
        Value::Nbt(bytes) => {
            writer.write_all(bytes)?;
            Ok(())
        }
    }
}

/// Reads one payload of a known shape.
fn read_value<R: Read>(
    reader: &mut R,
    opts: &NetDecodeOpts,
    shape: Shape,
) -> Result<Value, NetDecodeError> {
    Ok(match shape {
        Shape::Nothing => Value::Nothing,
        Shape::Number => Value::Number(VarInt::decode(reader, opts)?.0),
        Shape::Flag => Value::Flag(bool::decode(reader, opts)?),
        Shape::Colour => Value::Colour(i32::decode(reader, opts)?),
        Shape::Text => Value::Text(ferrumc_nbt::streaming::read_one(reader)?),
        Shape::Lines => {
            let count = VarInt::decode(reader, opts)?.0.max(0);
            let mut lines = Vec::with_capacity(count.min(256) as usize);
            for _ in 0..count {
                lines.push(ferrumc_nbt::streaming::read_one(reader)?);
            }
            Value::Lines(lines)
        }
        Shape::Enchantments => {
            let count = VarInt::decode(reader, opts)?.0.max(0);
            let mut held = Vec::with_capacity(count.min(256) as usize);
            for _ in 0..count {
                let id = VarInt::decode(reader, opts)?.0;
                let level = VarInt::decode(reader, opts)?.0;
                let Some(known) = u16::try_from(id)
                    .ok()
                    .and_then(|id| Enchantment::from_wire_id(id, ProtocolVersion::CURRENT))
                else {
                    continue;
                };
                held.push((known.id, u16::try_from(level).unwrap_or(0)));
            }
            Value::Enchantments(held)
        }
        Shape::Nbt => Value::Nbt(ferrumc_nbt::streaming::read_one(reader)?),
    })
}

/// A piece of text as the NBT it travels as.
fn as_nbt(text: &TextComponent) -> Vec<u8> {
    let mut bytes = Vec::new();
    ferrumc_nbt::NBT::new(text.clone())
        .encode(&mut bytes, &NetEncodeOpts::default())
        .expect("a text component writes to a buffer");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrumc_net_codec::encode::Framing;
    use ferrumc_text::ComponentBuilder;
    use std::io::Cursor;

    fn there_and_back(components: &Components, version: ProtocolVersion) -> Components {
        let mut bytes = Vec::new();
        components
            .encode(&mut bytes, &NetEncodeOpts::new(Framing::None, version))
            .expect("components write to a buffer");
        let mut reader = Cursor::new(&bytes);
        let read = Components::decode(&mut reader, &NetDecodeOpts::default())
            .expect("what was written reads back");
        assert_eq!(
            reader.position() as usize,
            bytes.len(),
            "the whole of it was read, and nothing was left to be read as something else"
        );
        read
    }

    fn a_named_and_enchanted_sword() -> Components {
        let mut components = Components::none();
        components.set_name(&ComponentBuilder::text("Glamdring").build());
        components.set(ComponentType::Damage, Value::Number(37));
        let sharpness = Enchantment::from_name("sharpness").expect("it is an enchantment");
        components.set(
            ComponentType::Enchantments,
            Value::Enchantments(vec![(sharpness.id, 5)]),
        );
        components
    }

    /// The phase's own question: does a named, enchanted, damaged item survive a round trip?
    #[test]
    fn a_named_enchanted_damaged_sword_survives_a_round_trip() {
        let sword = a_named_and_enchanted_sword();
        assert_eq!(there_and_back(&sword, ProtocolVersion::CURRENT), sword);
    }

    #[test]
    fn nothing_at_all_is_two_zeroes() {
        let mut bytes = Vec::new();
        Components::none()
            .encode(
                &mut bytes,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT),
            )
            .expect("nothing writes to a buffer");
        assert_eq!(bytes, vec![0, 0], "no components set, and none taken away");
    }

    #[test]
    fn a_component_taken_away_is_said_outright() {
        let mut components = Components::none();
        components.remove(ComponentType::Enchantments);
        assert_eq!(
            there_and_back(&components, ProtocolVersion::CURRENT)
                .removed()
                .collect::<Vec<_>>(),
            vec![ComponentType::Enchantments]
        );
    }

    #[test]
    fn setting_one_that_was_taken_away_puts_it_back() {
        let mut components = Components::none();
        components.remove(ComponentType::Damage);
        components.set(ComponentType::Damage, Value::Number(1));
        assert_eq!(components.removed().count(), 0);
        assert_eq!(components.damage(), 1);
    }

    #[test]
    fn two_stacks_built_in_different_orders_are_the_same_stack() {
        // Which is what lets them merge.
        let mut one = Components::none();
        one.set(ComponentType::Damage, Value::Number(3));
        one.set(ComponentType::Unbreakable, Value::Nothing);

        let mut other = Components::none();
        other.set(ComponentType::Unbreakable, Value::Nothing);
        other.set(ComponentType::Damage, Value::Number(3));

        assert_eq!(one, other);
    }

    /// An enchantment travels as a place in the reader's own registry, and `lunge` moved
    /// twenty-one of them in 26.1.
    #[test]
    fn an_enchantment_is_written_as_the_reading_client_numbers_it() {
        let sword = a_named_and_enchanted_sword();
        let bytes_for = |version| {
            let mut bytes = Vec::new();
            sword
                .encode(&mut bytes, &NetEncodeOpts::new(Framing::None, version))
                .expect("it writes");
            bytes
        };
        assert_ne!(
            bytes_for(ProtocolVersion::V26_2),
            bytes_for(ProtocolVersion::V1_21)
        );
    }

    /// The whole point of translating only when writing: what an older client cannot be shown is
    /// left out of *its* bytes and nowhere else. `lunge` arrived in 26.1.
    #[test]
    fn an_enchantment_an_older_client_cannot_see_is_still_on_the_sword() {
        let lunge = Enchantment::from_name("lunge").expect("it is an enchantment");
        let sharpness = Enchantment::from_name("sharpness").expect("it is an enchantment");

        let mut sword = Components::none();
        sword.set(
            ComponentType::Enchantments,
            Value::Enchantments(vec![(sharpness.id, 5), (lunge.id, 2)]),
        );

        // An older client is shown only what it has.
        let mut old = Vec::new();
        sword
            .encode(
                &mut old,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::V1_21),
            )
            .expect("it writes");
        let mut newer = Vec::new();
        sword
            .encode(
                &mut newer,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::V26_2),
            )
            .expect("it writes");
        assert!(old.len() < newer.len(), "one enchantment fewer went out");

        // And the sword still has both, because sending never touched it.
        assert_eq!(
            sword.get(ComponentType::Enchantments),
            Some(&Value::Enchantments(vec![(sharpness.id, 5), (lunge.id, 2)])),
            "nothing was lost by showing it to someone who cannot see it"
        );
    }

    /// The same for a whole component: `weapon` arrived in 26.1.
    #[test]
    fn a_component_an_older_client_cannot_see_is_still_on_the_stack() {
        let mut stack = Components::none();
        stack.set(ComponentType::Weapon, Value::Nothing);
        stack.set(ComponentType::Damage, Value::Number(3));

        let mut old = Vec::new();
        stack
            .encode(
                &mut old,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::V1_21),
            )
            .expect("it writes");

        assert_eq!(old[0], 1, "one component reached the older client");
        assert!(
            stack.get(ComponentType::Weapon).is_some(),
            "and the stack still has the other"
        );
    }

    #[test]
    fn a_component_a_client_has_never_heard_of_is_left_out_and_not_counted() {
        // `weapon` arrived in 26.1, so nothing older has it.
        let mut components = Components::none();
        components.set(ComponentType::Damage, Value::Number(1));
        components.set(ComponentType::Weapon, Value::Nothing);

        let mut old = Vec::new();
        components
            .encode(
                &mut old,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::V1_21),
            )
            .expect("it writes");
        assert_eq!(old[0], 1, "one component reached the older client");
    }

    #[test]
    fn a_component_with_no_known_shape_is_refused_rather_than_stepped_over() {
        // Nothing carries a length, so a reader that guesses reads the rest as nonsense.
        let mut bytes = vec![1, 0];
        let unknown = ComponentType::Tool
            .wire_id(ProtocolVersion::CURRENT)
            .expect("this version has it");
        bytes.push(u8::try_from(unknown).expect("a small number"));
        bytes.extend_from_slice(&[0xDE, 0xAD]);

        let mut reader = Cursor::new(&bytes);
        assert!(Components::decode(&mut reader, &NetDecodeOpts::default()).is_err());
    }

    #[test]
    fn something_this_server_cannot_read_still_survives_being_carried() {
        // Custom data is a compound nothing here looks inside, kept as it arrived.
        let mut components = Components::none();
        let mut compound = vec![10u8, 3];
        compound.extend_from_slice(&2u16.to_be_bytes());
        compound.extend_from_slice(b"id");
        compound.extend_from_slice(&9i32.to_be_bytes());
        compound.push(0);
        components.set(ComponentType::CustomData, Value::Nbt(compound.clone()));

        let back = there_and_back(&components, ProtocolVersion::CURRENT);
        assert_eq!(
            back.get(ComponentType::CustomData),
            Some(&Value::Nbt(compound))
        );
    }
}
