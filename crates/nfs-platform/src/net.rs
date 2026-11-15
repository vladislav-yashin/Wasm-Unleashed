//! The browser's datagram transport: a WebSocket to the UDP relay.
//!
//! A browser has no UDP socket, so [`RelaySocket`] speaks to `nfs-relay` over a WebSocket and that
//! relay forwards the bytes to the game host's UDP port. One WebSocket is one peer. This module only
//! moves opaque frames and reports connection state; the game's framing lives in
//! `nfs_net::relay`, because the relay server and the game share it and `nfs-platform` must not
//! depend on `nfs-net` (the dependency goes the other way).
//!
//! The WebSocket itself lives in `packaging/web/net.mjs` (`globalThis.nfsNet*`). Keeping it there
//! means the Rust side stays synchronous: [`RelaySocket::poll`] drains a queue that the module's
//! `onmessage` fills, exactly as the native `UdpSocket` drains its buffer. Nothing blocks the game
//! loop and no thread or timer is created.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;

/// How many frames the bridge keeps while the WebSocket is opening. The session's first second is
/// a repeated `'j'`; a longer queue only delays a stale datagram.
pub const MAX_QUEUED: usize = 64;

#[wasm_bindgen::prelude::wasm_bindgen(inline_js = "\
export function nfsNetAvailable(){return typeof globalThis.nfsNetOpen === 'function' ? 1 : 0} \
export function nfsNetOpen(target){try{return globalThis.nfsNetOpen ? globalThis.nfsNetOpen(target) : 0}catch(e){console.error(e);return 0}} \
export function nfsNetSend(bytes){try{return globalThis.nfsNetSend ? globalThis.nfsNetSend(bytes) : 0}catch(e){console.error(e);return 0}} \
export function nfsNetRecv(){try{return globalThis.nfsNetRecv ? globalThis.nfsNetRecv() : []}catch(e){console.error(e);return []}} \
export function nfsNetStatus(){try{return globalThis.nfsNetStatus ? globalThis.nfsNetStatus() : -1}catch(e){return -1}} \
export function nfsNetClose(id){try{globalThis.nfsNetClose?.(id)}catch(e){console.error(e)}}")]
extern "C" {
    fn nfsNetAvailable() -> i32;
    fn nfsNetOpen(target: &str) -> u32;
    fn nfsNetSend(bytes: &[u8]) -> i32;
    fn nfsNetRecv() -> js_sys::Array;
    fn nfsNetStatus() -> i32;
    fn nfsNetClose(id: u32);
}

/// Whether `packaging/web/net.mjs` installed its bridge. A page without it cannot join a game, and
/// the game says so in the log instead of pretending to connect for sixteen seconds.
pub fn available() -> bool {
    nfsNetAvailable() != 0
}

/// One WebSocket, bound to one relay destination. Frames the relay has sent wait in `incoming`
/// until the game's next poll.
pub struct RelaySocket {
    /// The generation number `net.mjs` returned, so dropping an idle lobby socket cannot close a
    /// live session's WebSocket.
    id: u32,
    incoming: RefCell<VecDeque<Vec<u8>>>,
}

impl RelaySocket {
    /// Open a relay connection to `target`. The WebSocket handshake is asynchronous; the game's
    /// first datagrams are queued by `net.mjs` until it opens.
    pub fn connect(target: SocketAddr) -> io::Result<Self> {
        if !available() {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "the browser relay bridge is not installed"));
        }
        let id = nfsNetOpen(&target.to_string());
        if id == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("cannot open a relay connection to {target}")));
        }
        Ok(Self { id, incoming: RefCell::new(VecDeque::new()) })
    }

    /// A socket for a lobby that only needs a live object: it never opens a WebSocket and its sends
    /// go nowhere. The peer-to-peer screen's typed address does not use browsing.
    pub fn idle() -> Self {
        Self { id: 0, incoming: RefCell::new(VecDeque::new()) }
    }

    /// Queue or send one relay frame. `false` means "not connected (yet)"; the game ignores send
    /// failures the same way it ignores a lost UDP datagram.
    pub fn send(&self, frame: &[u8]) -> bool {
        self.id != 0 && nfsNetSend(frame) != 0
    }

    /// Whether the WebSocket is open: 0 connecting, 1 open, anything else closed/failed.
    pub fn open(&self) -> bool {
        self.id != 0 && nfsNetStatus() == 1
    }

    /// The next frame the relay sent, or `None` when the queue is empty.
    pub fn poll(&self) -> io::Result<Option<Vec<u8>>> {
        if self.id == 0 {
            return Ok(None);
        }
        {
            let frames = nfsNetRecv();
            let mut incoming = self.incoming.borrow_mut();
            for i in 0..frames.length() {
                if incoming.len() >= MAX_QUEUED {
                    break;
                }
                incoming.push_back(js_sys::Uint8Array::new(&frames.get(i)).to_vec());
            }
        }
        Ok(self.incoming.borrow_mut().pop_front())
    }

}

impl Drop for RelaySocket {
    fn drop(&mut self) {
        if self.id != 0 {
            nfsNetClose(self.id);
        }
    }
}
