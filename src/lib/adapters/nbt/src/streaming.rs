//! Reading exactly one NBT value out of a stream that carries more after it.
//!
//! The parser this crate is built on takes a slice and reads a whole document. That is right for a
//! packet whose last field is NBT and wrong for anything that carries several: an item stack's
//! components sit one after another with no lengths between them, so reading one has to stop
//! exactly where it ends.
//!
//! What comes back is the bytes, which can then be handed to the slice parser. Walking the shape
//! once to find its end and once to read it is cheaper than it sounds — a component's worth of NBT
//! is a few dozen bytes — and it keeps the two parsers from having to agree on anything but the
//! format.

use std::io::{Error, ErrorKind, Read, Result};

/// The tag that closes a compound.
const END: u8 = 0;

/// How deep a document may nest before it is treated as malformed.
///
/// A stream is not to be trusted: without this, a compound that opens forever is a stack overflow
/// rather than an error.
const DEEPEST: usize = 512;

/// Reads one NBT value, in the form the network uses: a tag, then its payload, and no name in
/// front of it.
///
/// Returns the bytes it read, tag and all, which is what the slice parser expects.
pub fn read_one<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let tag = byte(reader, &mut out)?;
    if tag != END {
        payload(reader, &mut out, tag, 0)?;
    }
    Ok(out)
}

/// One byte, kept.
fn byte<R: Read>(reader: &mut R, out: &mut Vec<u8>) -> Result<u8> {
    let mut one = [0u8; 1];
    reader.read_exact(&mut one)?;
    out.push(one[0]);
    Ok(one[0])
}

/// This many bytes, kept.
fn take<R: Read>(reader: &mut R, out: &mut Vec<u8>, count: usize) -> Result<()> {
    let start = out.len();
    out.resize(start + count, 0);
    reader.read_exact(&mut out[start..])?;
    Ok(())
}

/// A big-endian count, kept, and refused if it is silly.
fn count<R: Read>(reader: &mut R, out: &mut Vec<u8>, wide: bool) -> Result<usize> {
    let start = out.len();
    take(reader, out, if wide { 4 } else { 2 })?;
    let read = if wide {
        i64::from(i32::from_be_bytes([
            out[start],
            out[start + 1],
            out[start + 2],
            out[start + 3],
        ]))
    } else {
        i64::from(u16::from_be_bytes([out[start], out[start + 1]]))
    };
    usize::try_from(read).map_err(|_| Error::new(ErrorKind::InvalidData, "a negative NBT length"))
}

/// One payload of a given tag.
fn payload<R: Read>(reader: &mut R, out: &mut Vec<u8>, tag: u8, depth: usize) -> Result<()> {
    if depth > DEEPEST {
        return Err(Error::new(ErrorKind::InvalidData, "NBT nested too deeply"));
    }
    match tag {
        1 => take(reader, out, 1),
        2 => take(reader, out, 2),
        3 | 5 => take(reader, out, 4),
        4 | 6 => take(reader, out, 8),
        // A byte array, a string, an int array and a long array are all a length and then that
        // many of something.
        7 => {
            let len = count(reader, out, true)?;
            take(reader, out, len)
        }
        8 => {
            let len = count(reader, out, false)?;
            take(reader, out, len)
        }
        11 => {
            let len = count(reader, out, true)?;
            take(reader, out, len * 4)
        }
        12 => {
            let len = count(reader, out, true)?;
            take(reader, out, len * 8)
        }
        // A list is one tag and a count, then that many payloads of it and no tags between them.
        9 => {
            let of = byte(reader, out)?;
            let len = count(reader, out, true)?;
            if of == END && len > 0 {
                return Err(Error::new(ErrorKind::InvalidData, "a list of nothing"));
            }
            for _ in 0..len {
                payload(reader, out, of, depth + 1)?;
            }
            Ok(())
        }
        // A compound is a tag, a name and a payload over and over until the tag that ends it.
        10 => loop {
            let of = byte(reader, out)?;
            if of == END {
                return Ok(());
            }
            let name = count(reader, out, false)?;
            take(reader, out, name)?;
            payload(reader, out, of, depth + 1)?;
        },
        other => Err(Error::new(
            ErrorKind::InvalidData,
            format!("no such NBT tag: {other}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A compound holding one int under one name, in the network form.
    fn a_compound() -> Vec<u8> {
        let mut out = vec![10]; // a compound, with no name in front of it
        out.push(3); // an int
        out.extend_from_slice(&2u16.to_be_bytes()); // named
        out.extend_from_slice(b"id");
        out.extend_from_slice(&7i32.to_be_bytes());
        out.push(END);
        out
    }

    #[test]
    fn one_value_is_read_and_nothing_after_it() {
        let mut stream = a_compound();
        let trailing = [0xAB, 0xCD];
        stream.extend_from_slice(&trailing);

        let mut reader = Cursor::new(&stream);
        let read = read_one(&mut reader).expect("a compound reads");
        assert_eq!(read, a_compound());

        let mut rest = Vec::new();
        reader
            .read_to_end(&mut rest)
            .expect("the rest is still there");
        assert_eq!(rest, trailing, "and was not eaten");
    }

    #[test]
    fn several_in_a_row_each_stop_where_they_end() {
        let mut stream = a_compound();
        stream.extend_from_slice(&a_compound());
        let mut reader = Cursor::new(&stream);

        assert_eq!(read_one(&mut reader).expect("the first"), a_compound());
        assert_eq!(read_one(&mut reader).expect("the second"), a_compound());
    }

    #[test]
    fn nothing_at_all_is_one_byte() {
        let mut reader = Cursor::new(vec![END, 0x42]);
        assert_eq!(read_one(&mut reader).expect("an end reads"), vec![END]);
    }

    #[test]
    fn a_list_of_compounds_reads() {
        let mut stream = vec![9, 10]; // a list of compounds
        stream.extend_from_slice(&2i32.to_be_bytes());
        for _ in 0..2 {
            stream.push(3); // an int
            stream.extend_from_slice(&1u16.to_be_bytes());
            stream.extend_from_slice(b"n");
            stream.extend_from_slice(&1i32.to_be_bytes());
            stream.push(END);
        }
        let whole = stream.len();
        stream.push(0x99);

        let mut reader = Cursor::new(&stream);
        assert_eq!(read_one(&mut reader).expect("a list reads").len(), whole);
    }

    #[test]
    fn a_tag_that_does_not_exist_is_refused_rather_than_guessed_at() {
        let mut reader = Cursor::new(vec![99]);
        assert!(read_one(&mut reader).is_err());
    }

    #[test]
    fn something_that_opens_forever_is_refused_rather_than_crashing() {
        // A stream is not to be trusted: a thousand opened compounds and no ends.
        let stream = vec![10u8; 2000];
        let mut reader = Cursor::new(&stream);
        assert!(read_one(&mut reader).is_err());
    }

    #[test]
    fn a_negative_length_is_refused() {
        let mut stream = vec![7]; // a byte array
        stream.extend_from_slice(&(-1i32).to_be_bytes());
        let mut reader = Cursor::new(&stream);
        assert!(read_one(&mut reader).is_err());
    }
}
