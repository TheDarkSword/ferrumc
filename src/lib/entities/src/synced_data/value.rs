//! The values a client is told about an entity, and how they reach it.
//!
//! Every value carries its own shape, and the shape a field expects is fixed by the entity type it
//! belongs to. A [`Field`] names one of them at the type level, so a caller writes a health as a
//! float and a pose as a pose without ever naming the number it sits at.

use super::generated::{
    Arm, ArmadilloState, CopperGolemState, Direction, Pose, SnifferState, WeatheringState,
};
use ferrumc_inventories::slot::InventorySlot;
use ferrumc_nbt::{NBTSerializable, NBTSerializeOptions};
use ferrumc_net_codec::encode::errors::NetEncodeError;
use ferrumc_net_codec::encode::{NetEncode, NetEncodeOpts};
use ferrumc_net_codec::net_types::network_position::NetworkPosition;
use ferrumc_net_codec::net_types::var_int::VarInt;
use ferrumc_net_codec::net_types::var_long::VarLong;
use ferrumc_text::TextComponent;
use std::io::Write;
use std::marker::PhantomData;

/// One value a client reads off an entity.
///
/// The variants are the shapes the server writes. A kind of value nothing writes yet keeps the
/// bytes the game itself wrote for it, so an entity still reaches a client whole; it gets a shape
/// of its own when something needs to set one.
#[derive(Debug, Clone, PartialEq)]
pub enum DataValue {
    Byte(u8),
    Int(i32),
    Long(i64),
    Float(f32),
    Boolean(bool),
    Text(String),
    Component(TextComponent),
    OptionalComponent(Option<TextComponent>),
    Item(InventorySlot),
    Rotations([f32; 3]),
    Vector3([f32; 3]),
    Quaternion([f32; 4]),
    BlockPos(NetworkPosition),
    OptionalBlockPos(Option<NetworkPosition>),
    Direction(Direction),
    Pose(Pose),
    Arm(Arm),
    SnifferState(SnifferState),
    ArmadilloState(ArmadilloState),
    CopperGolemState(CopperGolemState),
    WeatheringState(WeatheringState),
    /// Written exactly as the game wrote it, for a kind of value the server does not model yet.
    Raw(&'static [u8]),
}

impl NetEncode for DataValue {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        let opts = &opts.nested();
        match self {
            Self::Byte(value) => value.encode(writer, opts),
            Self::Int(value) => VarInt::new(*value).encode(writer, opts),
            Self::Long(value) => VarLong::new(*value).encode(writer, opts),
            Self::Float(value) => value.encode(writer, opts),
            Self::Boolean(value) => value.encode(writer, opts),
            Self::Text(value) => value.encode(writer, opts),
            Self::Component(value) => {
                value.serialize(writer, &NBTSerializeOptions::Network);
                Ok(())
            }
            Self::OptionalComponent(value) => match value {
                Some(component) => {
                    true.encode(writer, opts)?;
                    component.serialize(writer, &NBTSerializeOptions::Network);
                    Ok(())
                }
                None => false.encode(writer, opts),
            },
            Self::Item(value) => value.encode(writer, opts),
            Self::Rotations(axes) | Self::Vector3(axes) => {
                for axis in axes {
                    axis.encode(writer, opts)?;
                }
                Ok(())
            }
            Self::Quaternion(axes) => {
                for axis in axes {
                    axis.encode(writer, opts)?;
                }
                Ok(())
            }
            Self::BlockPos(value) => value.encode(writer, opts),
            Self::OptionalBlockPos(value) => match value {
                Some(pos) => {
                    true.encode(writer, opts)?;
                    pos.encode(writer, opts)
                }
                None => false.encode(writer, opts),
            },
            Self::Direction(value) => VarInt::new(value.wire_id()).encode(writer, opts),
            Self::Pose(value) => VarInt::new(value.wire_id()).encode(writer, opts),
            Self::Arm(value) => VarInt::new(value.wire_id()).encode(writer, opts),
            Self::SnifferState(value) => VarInt::new(value.wire_id()).encode(writer, opts),
            Self::ArmadilloState(value) => VarInt::new(value.wire_id()).encode(writer, opts),
            Self::CopperGolemState(value) => VarInt::new(value.wire_id()).encode(writer, opts),
            Self::WeatheringState(value) => VarInt::new(value.wire_id()).encode(writer, opts),
            Self::Raw(bytes) => writer.write_all(bytes).map_err(NetEncodeError::from),
        }
    }

    async fn encode_async<W: tokio::io::AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        // Values are a handful of bytes each, so one buffer beats an await per field.
        let mut buffer = Vec::new();
        self.encode(&mut buffer, opts)?;
        buffer.encode_async(writer, &opts.nested()).await
    }
}

/// One field of an entity, named rather than numbered.
///
/// The index a field sits at is fixed by the class that declared it, so a field of a given name is
/// in the same place on every entity type that inherits it. Carrying the value's shape as a type
/// parameter is what stops a health being written where a pose is read.
pub struct Field<T> {
    index: u8,
    // The value shape is what a field promises, not what it holds, so the marker is a producer.
    shape: PhantomData<fn() -> T>,
}

impl<T> Field<T> {
    /// The field at `index`, as the game numbers them.
    #[must_use]
    pub const fn at(index: u8) -> Self {
        Self {
            index,
            shape: PhantomData,
        }
    }

    /// Where this field sits, in the server's own version's terms.
    #[must_use]
    pub const fn index(self) -> u8 {
        self.index
    }
}

impl<T> Clone for Field<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Field<T> {}

/// A shape a field can hold.
pub trait DataField: Sized + PartialEq {
    /// This value as one a client can be told about.
    fn into_value(self) -> DataValue;

    /// This shape back out of a value, or nothing if the value is a different shape.
    fn from_value(value: &DataValue) -> Option<&Self>;
}

/// Implements the conversion both ways for shapes that map onto one variant.
macro_rules! holds {
    ($($shape:ty => $variant:ident),* $(,)?) => {
        $(
            impl DataField for $shape {
                fn into_value(self) -> DataValue {
                    DataValue::$variant(self)
                }

                fn from_value(value: &DataValue) -> Option<&Self> {
                    match value {
                        DataValue::$variant(held) => Some(held),
                        _ => None,
                    }
                }
            }
        )*
    };
}

holds! {
    u8 => Byte,
    i32 => Int,
    i64 => Long,
    f32 => Float,
    bool => Boolean,
    String => Text,
    TextComponent => Component,
    Option<TextComponent> => OptionalComponent,
    InventorySlot => Item,
    NetworkPosition => BlockPos,
    Option<NetworkPosition> => OptionalBlockPos,
    Direction => Direction,
    Pose => Pose,
    Arm => Arm,
    SnifferState => SnifferState,
    ArmadilloState => ArmadilloState,
    CopperGolemState => CopperGolemState,
    WeatheringState => WeatheringState,
}

// Rotations and a plain vector are both three floats, so which one an array means is decided by
// the field it is written to rather than by the array itself. Rotations are the older and far more
// common of the two, so a bare array is one.
impl DataField for [f32; 3] {
    fn into_value(self) -> DataValue {
        DataValue::Rotations(self)
    }

    fn from_value(value: &DataValue) -> Option<&Self> {
        match value {
            DataValue::Rotations(held) | DataValue::Vector3(held) => Some(held),
            _ => None,
        }
    }
}

impl DataField for [f32; 4] {
    fn into_value(self) -> DataValue {
        DataValue::Quaternion(self)
    }

    fn from_value(value: &DataValue) -> Option<&Self> {
        match value {
            DataValue::Quaternion(held) => Some(held),
            _ => None,
        }
    }
}
