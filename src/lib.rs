#![deny(warnings)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate byteorder;
#[cfg(feature = "std")]
extern crate std as core;

#[cfg(test)]
extern crate rayon;

use byteorder::{BigEndian, ByteOrder};
use core::result;
use core::str;

pub type PacketTypeFlags = u8;
pub type PacketId = u16;

pub mod error;
pub use error::Error;

#[macro_use]
pub mod status;
pub use status::Status;

pub mod header;
pub use header::Header;

pub mod connect;
pub use connect::Connect;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum QoS {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

impl QoS {
    pub fn from_u8(qos: u8) -> Result<QoS> {
        match qos {
            0 => Ok(QoS::AtMostOnce),
            1 => Ok(QoS::AtLeastOnce),
            2 => Ok(QoS::ExactlyOnce),
            _ => Err(Error::InvalidQoS),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PacketType {
    Connect,
    Connack,
    Publish,
    Puback,
    Pubrec,
    Pubrel,
    Pubcomp,
    Subscribe,
    Suback,
    Unsubscribe,
    Unsuback,
    Pingreq,
    Pingresp,
    Disconnect,
}

pub type Result<T> = result::Result<T, Error>;

pub fn parse_string(bytes: &[u8]) -> Result<Status<&str>> {
    // we need at least the 2 bytes to figure out length of the utf-8 encoded
    // string in bytes
    if bytes.len() < 2 {
        return Ok(Status::Partial);
    }

    let len = BigEndian::read_u16(bytes);
    if bytes.len() - 2 < len as usize {
        return Ok(Status::Partial);
    }

    // Rust string slices are never in the code point range 0xD800 and
    // 0xDFFF which takes care of requirement MQTT-1.5.3-1. str::from_utf8
    // will fail if those code points are found in "bytes".
    //
    // Rust utf-8 decoding also takes care of MQTT-1.5.3-3. U+FEFF does not
    // get ignored/stripped off.
    let val = str::from_utf8(&bytes[2..(len + 2) as usize])?;

    // Requirement MQTT-1.5.3-2 requires that there be no U+0000 code points
    // in the string.
    if val.chars().any(|ch| ch == '\u{0000}') {
        Err(Error::Utf8)
    } else {
        Ok(Status::Complete(val))
    }
}

pub fn parse_len_prefixed_bytes(bytes: &[u8]) -> Result<Status<&[u8]>> {
    // we need at least the 2 bytes to figure out length of the payload
    if bytes.len() < 2 {
        return Ok(Status::Partial);
    }

    let len = BigEndian::read_u16(bytes);
    if bytes.len() - 2 < len as usize {
        return Ok(Status::Partial);
    }

    Ok(Status::Complete(&bytes[2..(len + 2) as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse_string {
        use super::*;
        use std::io::{Cursor, Write};

        use byteorder::WriteBytesExt;

        #[test]
        fn small_buffer() {
            assert_eq!(Status::Partial, parse_string(&[]).unwrap());
            assert_eq!(Status::Partial, parse_string(&[0]).unwrap());

            let mut buf = [0u8; 2];
            BigEndian::write_u16(&mut buf, 16);
            assert_eq!(Status::Partial, parse_string(&buf).unwrap());
        }

        #[test]
        fn empty_str() {
            let buf = [0u8; 2];
            assert_eq!(Status::Complete(""), parse_string(&buf).unwrap());
        }

        #[test]
        fn parse_str() {
            let inp = "don't panic!";
            let mut buf = Cursor::new(Vec::new());
            buf.write_u16::<BigEndian>(inp.len() as u16).unwrap();
            buf.write(inp.as_bytes()).unwrap();
            assert_eq!(
                Status::Complete(inp),
                parse_string(buf.get_ref().as_ref()).unwrap()
            );
        }

        #[test]
        fn invalid_utf8() {
            let inp = [0, 159, 146, 150];
            let mut buf = Cursor::new(Vec::new());
            buf.write_u16::<BigEndian>(inp.len() as u16).unwrap();
            buf.write(&inp).unwrap();
            assert_eq!(Err(Error::Utf8), parse_string(buf.get_ref().as_ref()));
        }

        #[test]
        fn null_utf8() {
            let inp = format!("don't {} panic!", '\u{0000}');
            let mut buf = Cursor::new(Vec::new());
            buf.write_u16::<BigEndian>(inp.len() as u16).unwrap();
            buf.write(inp.as_bytes()).unwrap();
            assert_eq!(Err(Error::Utf8), parse_string(buf.get_ref().as_ref()));
        }
    }

    mod parse_len_prefixed_bytes {
        use super::*;
        use std::io::{Cursor, Write};

        use byteorder::WriteBytesExt;

        #[test]
        fn small_buffer() {
            assert_eq!(Status::Partial, parse_len_prefixed_bytes(&[]).unwrap());
            assert_eq!(Status::Partial, parse_len_prefixed_bytes(&[0]).unwrap());

            let mut buf = [0u8; 2];
            BigEndian::write_u16(&mut buf, 16);
            assert_eq!(Status::Partial, parse_len_prefixed_bytes(&buf).unwrap());
        }

        #[test]
        fn empty_bytes() {
            let buf = [0u8; 2];
            assert_eq!(
                Status::Complete(&buf[0..0]),
                parse_len_prefixed_bytes(&buf).unwrap()
            );
        }

        #[test]
        fn parse_bytes() {
            let inp = "don't panic!".as_bytes();
            let mut buf = Cursor::new(Vec::new());
            buf.write_u16::<BigEndian>(inp.len() as u16).unwrap();
            buf.write(inp).unwrap();
            assert_eq!(
                Status::Complete(inp),
                parse_len_prefixed_bytes(buf.get_ref().as_ref()).unwrap()
            );
        }
    }
}
