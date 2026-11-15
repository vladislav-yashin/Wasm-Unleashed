//! `nfs-relay` — the WebSocket to UDP gateway a browser needs to join a network game.
//!
//! Browsers cannot open UDP sockets, so the browser build speaks the original protocol through one
//! WebSocket per peer. This crate is the other end of that WebSocket: it validates the requested
//! destination, opens one connected UDP socket per connection and forwards the game's datagrams
//! unchanged. The framing is `nfs_net::relay`; the game's own bytes never change.

pub mod config;
pub mod server;
pub mod target;
