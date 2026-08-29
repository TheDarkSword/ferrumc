use crate::decode::errors::NetDecodeError;
use crate::decode::{NetDecode, NetDecodeOpts};
use crate::encode::errors::NetEncodeError;
use crate::encode::{NetEncode, NetEncodeOpts};
use crate::net_types::NetTypesError;
use std::io::{Read, Write};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const SEGMENT_BITS: i64 = 0x7F;
const CONTINUE_BIT: i64 = 0x80;

/// A variable-length signed 64-bit integer, encoded the same way as [`VarInt`](super::var_int::VarInt)
/// but over up to ten bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct VarLong(pub i64);

impl VarLong {
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> Result<(), NetTypesError> {
        let VarLong(mut val) = self;
        loop {
            if (val & !SEGMENT_BITS) == 0 {
                writer.write_all(&[val as u8])?;
                return Ok(());
            }
            writer.write_all(&[((val & SEGMENT_BITS) | CONTINUE_BIT) as u8])?;
            val = ((val as u64) >> 7) as i64;
        }
    }

    pub async fn write_async<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
    ) -> Result<(), NetTypesError> {
        let VarLong(mut val) = self;
        loop {
            if (val & !SEGMENT_BITS) == 0 {
                writer.write_all(&[val as u8]).await?;
                return Ok(());
            }
            writer
                .write_all(&[((val & SEGMENT_BITS) | CONTINUE_BIT) as u8])
                .await?;
            val = ((val as u64) >> 7) as i64;
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> Result<Self, NetTypesError> {
        let mut value: i64 = 0;
        let mut shift = 0;
        loop {
            let mut byte = [0u8; 1];
            reader.read_exact(&mut byte)?;
            value |= i64::from(byte[0] & SEGMENT_BITS as u8) << shift;
            if byte[0] & CONTINUE_BIT as u8 == 0 {
                return Ok(Self(value));
            }
            shift += 7;
            if shift >= 64 {
                return Err(NetTypesError::InvalidVarLong);
            }
        }
    }
}

impl From<i64> for VarLong {
    fn from(value: i64) -> Self {
        Self::new(value)
    }
}

impl From<u64> for VarLong {
    fn from(value: u64) -> Self {
        Self::new(value as i64)
    }
}

impl NetEncode for VarLong {
    fn encode<W: Write>(
        &self,
        writer: &mut W,
        _opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        self.write(writer)
            .map_err(|e| NetEncodeError::ExternalError(e.into()))
    }

    async fn encode_async<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        _opts: &NetEncodeOpts,
    ) -> Result<(), NetEncodeError> {
        self.write_async(writer)
            .await
            .map_err(|e| NetEncodeError::ExternalError(e.into()))
    }
}

impl NetDecode for VarLong {
    fn decode<R: Read>(reader: &mut R, _opts: &NetDecodeOpts) -> Result<Self, NetDecodeError> {
        Self::read(reader).map_err(|e| NetDecodeError::ExternalError(e.into()))
    }

    async fn decode_async<R: AsyncRead + Unpin>(
        reader: &mut R,
        _opts: &NetDecodeOpts,
    ) -> Result<Self, NetDecodeError> {
        let mut value: i64 = 0;
        let mut shift = 0;
        loop {
            let byte = reader
                .read_u8()
                .await
                .map_err(|e| NetDecodeError::ExternalError(e.into()))?;
            value |= i64::from(byte & SEGMENT_BITS as u8) << shift;
            if byte & CONTINUE_BIT as u8 == 0 {
                return Ok(Self(value));
            }
            shift += 7;
            if shift >= 64 {
                return Err(NetDecodeError::ExternalError(
                    NetTypesError::InvalidVarLong.into(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_boundary_values() {
        for value in [
            0,
            1,
            -1,
            127,
            128,
            i64::from(i32::MAX),
            i64::from(i32::MIN),
            i64::MAX,
            i64::MIN,
        ] {
            let mut buf = Vec::new();
            VarLong(value).write(&mut buf).expect("write succeeds");
            let decoded = VarLong::read(&mut buf.as_slice()).expect("read succeeds");
            assert_eq!(decoded.0, value, "round trip for {value}");
        }
    }

    #[test]
    fn matches_var_int_encoding_for_small_values() {
        use crate::net_types::var_int::VarInt;
        for value in [0i32, 1, 127, 128, 300, 16_383, 16_384] {
            let mut long_buf = Vec::new();
            let mut int_buf = Vec::new();
            VarLong(i64::from(value))
                .write(&mut long_buf)
                .expect("write succeeds");
            VarInt(value).write(&mut int_buf).expect("write succeeds");
            assert_eq!(long_buf, int_buf, "encoding differs for {value}");
        }
    }
}
