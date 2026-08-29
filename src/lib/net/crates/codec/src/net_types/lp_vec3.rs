//! A vector packed into as few bytes as the precision it needs.
//!
//! Movement and interaction points are sent constantly and rarely need the range a triple of
//! doubles gives them, so the three axes share one scale and are quantised to fifteen bits each. A
//! vector short enough to round to nothing is a single zero byte, which is what most of them are.

use crate::decode::errors::NetDecodeError;
use crate::decode::{NetDecode, NetDecodeOpts};
use crate::encode::errors::NetEncodeError;
use crate::encode::{NetEncode, NetEncodeOpts};
use crate::net_types::var_int::VarInt;
use std::io::{Read, Write};

/// Width of one axis once quantised.
const DATA_MASK: u64 = 0x7FFF;
/// The largest quantised value, one short of the mask so the range stays symmetric about zero.
const MAX_QUANTIZED: f64 = 32766.0;
/// The scale's low two bits share the first byte with the axes; anything larger sets a flag and
/// sends the rest of the scale as a varint.
const SCALE_MASK: u64 = 0b11;
const CONTINUATION: u64 = 0b100;
const X_OFFSET: u32 = 3;
const Y_OFFSET: u32 = 18;
const Z_OFFSET: u32 = 33;
/// Shorter than this rounds to nothing and is written as a single zero byte.
const ABS_MIN: f64 = 3.051_944_088_384_301E-5;
const ABS_MAX: f64 = 1.717_986_918_3E10;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LowPrecisionVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl LowPrecisionVec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[must_use]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    fn sanitize(value: f64) -> f64 {
        if value.is_nan() {
            0.0
        } else {
            value.clamp(-ABS_MAX, ABS_MAX)
        }
    }

    fn pack(value: f64) -> u64 {
        ((value * 0.5 + 0.5) * MAX_QUANTIZED).round() as u64
    }

    fn unpack(value: u64) -> f64 {
        ((value & DATA_MASK) as f64).min(MAX_QUANTIZED) * 2.0 / MAX_QUANTIZED - 1.0
    }
}

impl NetEncode for LowPrecisionVec3 {
    fn encode<W: Write>(&self, writer: &mut W, opts: &NetEncodeOpts) -> Result<(), NetEncodeError> {
        let (x, y, z) = (
            Self::sanitize(self.x),
            Self::sanitize(self.y),
            Self::sanitize(self.z),
        );
        // The scale covers the longest axis, so the quantised values are all a fraction of it.
        let longest = x.abs().max(y.abs()).max(z.abs());
        if longest < ABS_MIN {
            writer.write_all(&[0])?;
            return Ok(());
        }

        let scale = longest.ceil() as u64;
        let partial = scale & SCALE_MASK != scale;
        let markers = if partial {
            (scale & SCALE_MASK) | CONTINUATION
        } else {
            scale
        };
        let divisor = scale as f64;
        let buffer = markers
            | Self::pack(x / divisor) << X_OFFSET
            | Self::pack(y / divisor) << Y_OFFSET
            | Self::pack(z / divisor) << Z_OFFSET;

        writer.write_all(&[buffer as u8, (buffer >> 8) as u8])?;
        writer.write_all(&((buffer >> 16) as u32).to_be_bytes())?;
        if partial {
            VarInt::new((scale >> 2) as i32).encode(writer, opts)?;
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
        <W as tokio::io::AsyncWriteExt>::write_all(writer, &buffer).await?;
        Ok(())
    }
}

impl NetDecode for LowPrecisionVec3 {
    fn decode<R: Read>(reader: &mut R, opts: &NetDecodeOpts) -> Result<Self, NetDecodeError> {
        let mut lowest = [0u8; 1];
        reader.read_exact(&mut lowest)?;
        let lowest = u64::from(lowest[0]);
        if lowest == 0 {
            return Ok(Self::ZERO);
        }

        let mut rest = [0u8; 5];
        reader.read_exact(&mut rest)?;
        let middle = u64::from(rest[0]);
        let highest = u64::from(u32::from_be_bytes([rest[1], rest[2], rest[3], rest[4]]));
        let buffer = highest << 16 | middle << 8 | lowest;

        let mut scale = lowest & SCALE_MASK;
        if lowest & CONTINUATION != 0 {
            scale |= u64::from(VarInt::decode(reader, opts)?.0 as u32) << 2;
        }
        let scale = scale as f64;

        Ok(Self {
            x: Self::unpack(buffer >> X_OFFSET) * scale,
            y: Self::unpack(buffer >> Y_OFFSET) * scale,
            z: Self::unpack(buffer >> Z_OFFSET) * scale,
        })
    }

    async fn decode_async<R: tokio::io::AsyncRead + Unpin>(
        _reader: &mut R,
        _opts: &NetDecodeOpts,
    ) -> Result<Self, NetDecodeError> {
        unreachable!("packets are decoded from a buffered frame")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode::Framing;
    use crate::version::ProtocolVersion;

    fn round_trip(vector: LowPrecisionVec3) -> LowPrecisionVec3 {
        let mut buffer = Vec::new();
        vector
            .encode(
                &mut buffer,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT),
            )
            .expect("encodes");
        LowPrecisionVec3::decode(&mut buffer.as_slice(), &NetDecodeOpts::None).expect("decodes")
    }

    /// Anything shorter than the smallest representable step is a single zero byte, which is what
    /// a standing entity sends every tick.
    #[test]
    fn nothing_is_one_byte() {
        let mut buffer = Vec::new();
        LowPrecisionVec3::ZERO
            .encode(
                &mut buffer,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT),
            )
            .expect("encodes");
        assert_eq!(buffer, vec![0]);
        assert_eq!(round_trip(LowPrecisionVec3::ZERO), LowPrecisionVec3::ZERO);
    }

    /// Quantising loses precision but has to keep the value close, and keep the signs.
    #[test]
    fn a_vector_survives_the_round_trip() {
        for vector in [
            LowPrecisionVec3::new(1.0, -1.0, 0.5),
            LowPrecisionVec3::new(0.1, 0.2, -0.3),
            // Long enough that the scale no longer fits in two bits and continues as a varint.
            LowPrecisionVec3::new(120.0, -45.5, 3.25),
        ] {
            let back = round_trip(vector);
            let tolerance = vector.x.abs().max(vector.y.abs()).max(vector.z.abs()) / 1000.0;
            assert!(
                (back.x - vector.x).abs() <= tolerance
                    && (back.y - vector.y).abs() <= tolerance
                    && (back.z - vector.z).abs() <= tolerance,
                "{vector:?} came back as {back:?}"
            );
        }
    }

    /// A short vector fits in six bytes; a long one carries the rest of its scale after them.
    #[test]
    fn the_scale_continues_only_when_it_has_to() {
        let short = LowPrecisionVec3::new(1.0, 0.0, 0.0);
        let long = LowPrecisionVec3::new(120.0, 0.0, 0.0);
        let encode = |v: LowPrecisionVec3| {
            let mut buffer = Vec::new();
            v.encode(
                &mut buffer,
                &NetEncodeOpts::new(Framing::None, ProtocolVersion::CURRENT),
            )
            .expect("encodes");
            buffer
        };
        assert_eq!(encode(short).len(), 6);
        assert!(encode(long).len() > 6);
    }
}
