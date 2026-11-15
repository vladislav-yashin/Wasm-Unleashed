//! The WebSocket framing a browser uses to reach the original UDP protocol through a relay.
//!
//! A browser cannot open a UDP socket, so the browser build speaks the game's datagrams through a
//! WebSocket to [`nfs-relay`]. That relay opens one connected UDP socket per WebSocket and forwards
//! the bytes unchanged: one WebSocket is one peer as far as the game's session is concerned.
//!
//! The game's own datagram ([`crate::wire::Datagram`]) is opaque here — a tag byte and a payload, at
//! most [`MAX_DATAGRAM`] bytes. The relay only wraps it with the direction and the address:
//!
//! ```text
//! browser -> relay   [0x01][game datagram]
//! relay -> browser   [0x01][IPv4: 4][port: u16be][game datagram]
//! relay -> browser   [0x02][IPv4: 4][port: u16be]              the endpoint the host sees
//! ```
//!
//! One WebSocket is bound to one destination, named in the relay URL (`/relay?target=a.b.c.d:9442`),
//! so the browser's frames carry no address. Addresses are IPv4 only: the original protocol has no
//! room for an IPv6 record (`crate::wire::Addr`).
//!
//! The relay never interprets the game datagram. Keeping this codec here, rather than in the relay
//! crate, lets the browser transport and the relay share it and its tests.

use crate::wire::MAX_DATAGRAM;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use thiserror::Error;

/// A game datagram in one direction, with the source address on the way to the browser.
pub const FRAME_DATA: u8 = 1;
/// The UDP endpoint the relay's socket has towards the host, so the game can show it.
pub const FRAME_ENDPOINT: u8 = 2;

/// `FRAME_DATA` and the game datagram.
pub const CLIENT_HEADER: usize = 1;
/// `FRAME_DATA`, IPv4 (four bytes) and the port (two bytes).
pub const SERVER_HEADER: usize = 7;
/// `FRAME_ENDPOINT`, IPv4 and the port.
pub const ENDPOINT_SIZE: usize = 7;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RelayError {
    #[error("a relay frame cannot be empty")]
    Empty,
    #[error("unknown relay frame type {0}")]
    Unknown(u8),
    #[error("a relay frame is {0} bytes, at most {MAX_DATAGRAM} plus a header fit")]
    TooLong(usize),
    #[error("a relay frame is {0} bytes, shorter than its header")]
    Short(usize),
    #[error("the game datagram is {0} bytes, the protocol allows {MAX_DATAGRAM}")]
    BadDatagram(usize),
    #[error("the game datagram is empty; the tag byte is always there")]
    EmptyDatagram,
}

/// The browser's side of one frame: `[0x01][game datagram]`.
pub fn encode_client(datagram: &[u8]) -> Result<Vec<u8>, RelayError> {
    check_datagram(datagram)?;
    let mut out = Vec::with_capacity(CLIENT_HEADER + datagram.len());
    out.push(FRAME_DATA);
    out.extend_from_slice(datagram);
    Ok(out)
}

/// The relay's data frame: `[0x01][IPv4][port][game datagram]`.
pub fn encode_server(from: SocketAddr, datagram: &[u8]) -> Result<Vec<u8>, RelayError> {
    check_datagram(datagram)?;
    let SocketAddr::V4(v4) = from else {
        return Err(RelayError::BadDatagram(datagram.len()));
    };
    let mut out = Vec::with_capacity(SERVER_HEADER + datagram.len());
    out.push(FRAME_DATA);
    out.extend_from_slice(&v4.ip().octets());
    out.extend_from_slice(&v4.port().to_be_bytes());
    out.extend_from_slice(datagram);
    Ok(out)
}

/// The relay's endpoint frame, sent once when the UDP socket is connected.
pub fn encode_endpoint(addr: SocketAddr) -> Option<[u8; ENDPOINT_SIZE]> {
    let SocketAddr::V4(v4) = addr else { return None };
    let mut out = [0u8; ENDPOINT_SIZE];
    out[0] = FRAME_ENDPOINT;
    out[1..5].copy_from_slice(&v4.ip().octets());
    out[5..7].copy_from_slice(&v4.port().to_be_bytes());
    Some(out)
}

/// What the relay can send to a browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerFrame {
    /// One game datagram and the address it came from.
    Data { from: SocketAddr, datagram: Vec<u8> },
    /// The relay's own endpoint towards the host.
    Endpoint(SocketAddr),
}

/// Split one frame sent by the relay.
pub fn decode_server(frame: &[u8]) -> Result<ServerFrame, RelayError> {
    match frame.first().copied() {
        None => Err(RelayError::Empty),
        Some(FRAME_DATA) => {
            if frame.len() < SERVER_HEADER {
                return Err(RelayError::Short(frame.len()));
            }
            let datagram = &frame[SERVER_HEADER..];
            if datagram.is_empty() {
                return Err(RelayError::EmptyDatagram);
            }
            check_len(datagram.len())?;
            Ok(ServerFrame::Data { from: SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(frame[1], frame[2], frame[3], frame[4]),
                u16::from_be_bytes([frame[5], frame[6]]),
            )), datagram: datagram.to_vec() })
        }
        Some(FRAME_ENDPOINT) => {
            if frame.len() < ENDPOINT_SIZE {
                return Err(RelayError::Short(frame.len()));
            }
            Ok(ServerFrame::Endpoint(SocketAddr::V4(SocketAddrV4::new(
                Ipv4Addr::new(frame[1], frame[2], frame[3], frame[4]),
                u16::from_be_bytes([frame[5], frame[6]]),
            ))))
        }
        Some(other) => Err(RelayError::Unknown(other)),
    }
}

/// Split one frame sent by a browser: the game datagram.
pub fn decode_client(frame: &[u8]) -> Result<&[u8], RelayError> {
    match frame.first().copied() {
        None => Err(RelayError::Empty),
        Some(FRAME_DATA) => {
            let datagram = &frame[CLIENT_HEADER..];
            if datagram.is_empty() {
                return Err(RelayError::EmptyDatagram);
            }
            check_len(datagram.len())?;
            Ok(datagram)
        }
        Some(other) => Err(RelayError::Unknown(other)),
    }
}

fn check_len(len: usize) -> Result<(), RelayError> {
    if len > MAX_DATAGRAM { Err(RelayError::TooLong(len)) } else { Ok(()) }
}

fn check_datagram(datagram: &[u8]) -> Result<(), RelayError> {
    if datagram.is_empty() || datagram.len() > MAX_DATAGRAM { Err(RelayError::BadDatagram(datagram.len())) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FROM: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 5), 9442));

    #[test]
    fn a_client_frame_is_the_tag_byte() {
        let f = encode_client(b"=abc").unwrap();
        assert_eq!(f, [b'\x01', b'=', b'a', b'b', b'c']);
        assert_eq!(decode_client(&f).unwrap(), b"=abc");
        assert_eq!(decode_client(&[]), Err(RelayError::Empty));
        assert_eq!(decode_client(&[9, 1]), Err(RelayError::Unknown(9)));
    }

    #[test]
    fn a_frame_without_a_game_datagram_is_refused() {
        // The tag byte is part of the datagram, so a header alone is not a frame to forward.
        assert_eq!(decode_client(&[FRAME_DATA]), Err(RelayError::EmptyDatagram));
        let mut server = vec![0u8; SERVER_HEADER];
        server[0] = FRAME_DATA;
        assert_eq!(decode_server(&server), Err(RelayError::EmptyDatagram));
    }

    #[test]
    fn a_server_frame_carries_the_source_address() {
        let f = encode_server(FROM, b"jhello").unwrap();
        assert_eq!(&f[..7], &[1, 192, 168, 1, 5, 0x24, 0xE2]);
        assert_eq!(decode_server(&f).unwrap(), ServerFrame::Data { from: FROM, datagram: b"jhello".to_vec() });
    }

    #[test]
    fn an_endpoint_is_sent_once() {
        let f = encode_endpoint(FROM).unwrap();
        assert_eq!(decode_server(&f).unwrap(), ServerFrame::Endpoint(FROM));
        assert_eq!(decode_server(&f[..6]), Err(RelayError::Short(6)));
    }

    #[test]
    fn the_protocol_limits_are_enforced() {
        let full = vec![b'='; MAX_DATAGRAM];
        assert!(encode_client(&full).is_ok());
        assert_eq!(encode_client(&vec![0; MAX_DATAGRAM + 1]), Err(RelayError::BadDatagram(MAX_DATAGRAM + 1)));
        assert_eq!(encode_client(&[]), Err(RelayError::BadDatagram(0)));
        let mut f = vec![1u8; SERVER_HEADER + MAX_DATAGRAM + 1];
        f[0] = FRAME_DATA;
        assert_eq!(decode_server(&f), Err(RelayError::TooLong(MAX_DATAGRAM + 1)));
        let mut c = vec![1u8; CLIENT_HEADER + MAX_DATAGRAM + 1];
        c[0] = FRAME_DATA;
        assert_eq!(decode_client(&c), Err(RelayError::TooLong(MAX_DATAGRAM + 1)));
    }
}
