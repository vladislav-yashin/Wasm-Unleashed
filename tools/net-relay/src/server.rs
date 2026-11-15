//! The HTTP/WebSocket server and one relay connection.
//!
//! `/relay?target=a.b.c.d:port` upgrades, opens one connected UDP socket to the validated target,
//! sends the endpoint the host will see, and then forwards datagrams in both directions. `/healthz`
//! answers the deployment's health checks. Everything else is 404.

use crate::config::Config;
use crate::target::{Cidr, TargetPolicy};
use axum::body::Bytes;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use nfs_net::relay;
use nfs_net::wire::MAX_DATAGRAM;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, watch};

/// The largest frame the relay accepts or sends: one game datagram and its header.
const MAX_FRAME: usize = MAX_DATAGRAM + relay::SERVER_HEADER;

/// Per-address upgrade buckets kept before the quiet ones are forgotten. The map only holds
/// addresses that tried recently, so a long-running relay does not remember the internet.
const UPGRADE_BUCKETS: usize = 4096;
/// How long an untouched upgrade bucket stays in that map.
const UPGRADE_BUCKET_IDLE: Duration = Duration::from_secs(600);

/// Shared state for the router and every live connection.
pub struct AppState {
    pub config: Config,
    pub policy: TargetPolicy,
    active: AtomicUsize,
    per_ip: Mutex<HashMap<IpAddr, usize>>,
    per_target: Mutex<HashMap<SocketAddr, usize>>,
    /// One token bucket per address that tried to upgrade, so a handshake flood is bounded even
    /// when the relay is reached without the edge's request limit.
    upgrades: Mutex<HashMap<IpAddr, Rate>>,
    /// The deployment's bandwidth bill, shared by every connection.
    bandwidth: Mutex<Bandwidth>,
    /// Datagrams the budget refused, for the periodic log line.
    dropped: AtomicU64,
    /// Connections refused by a limit, for the periodic log line.
    refused: AtomicU64,
    /// Broadcast on SIGTERM/SIGINT so live sessions send a close frame and the server can stop.
    shutdown: broadcast::Sender<()>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let policy = config.policy();
        let bandwidth = Bandwidth::new(config.max_bytes_per_sec);
        let (shutdown, _) = broadcast::channel(16);
        Self {
            config,
            policy,
            active: AtomicUsize::new(0),
            per_ip: Mutex::new(HashMap::new()),
            per_target: Mutex::new(HashMap::new()),
            upgrades: Mutex::new(HashMap::new()),
            bandwidth: Mutex::new(bandwidth),
            dropped: AtomicU64::new(0),
            refused: AtomicU64::new(0),
            shutdown,
        }
    }

    /// Reserve one slot for a connection. Every refusal is the same `429`: the client may retry
    /// once a session ends, but nothing it sends can make room.
    fn acquire(self: &Arc<Self>, ip: IpAddr, target: SocketAddr) -> Result<ConnGuard, Admission> {
        if self.active.fetch_add(1, Ordering::SeqCst) >= self.config.max_connections {
            self.active.fetch_sub(1, Ordering::SeqCst);
            return Err(Admission::Global);
        }
        {
            let mut counts = self.per_ip.lock().unwrap_or_else(|e| e.into_inner());
            let count = counts.entry(ip).or_default();
            if *count >= self.config.max_connections_per_ip {
                drop(counts);
                self.active.fetch_sub(1, Ordering::SeqCst);
                return Err(Admission::PerIp);
            }
            *count += 1;
        }
        {
            let mut counts = self.per_target.lock().unwrap_or_else(|e| e.into_inner());
            let count = counts.entry(target).or_default();
            if *count >= self.config.max_connections_per_target {
                drop(counts);
                self.release_ip(ip);
                self.active.fetch_sub(1, Ordering::SeqCst);
                return Err(Admission::PerTarget);
            }
            *count += 1;
        }
        Ok(ConnGuard { state: self.clone(), ip, target })
    }

    fn release_ip(&self, ip: IpAddr) {
        if let Ok(mut counts) = self.per_ip.lock() {
            if let Some(count) = counts.get_mut(&ip) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    counts.remove(&ip);
                }
            }
        }
    }

    /// Charge one upgrade attempt to `ip`. `false` means the address is opening WebSockets too
    /// fast; the answer is `429` like a full slot, because waiting is the only remedy either way.
    fn admit_upgrade(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let mut buckets = self.upgrades.lock().unwrap_or_else(|e| e.into_inner());
        let allowed = buckets.entry(ip).or_insert_with(|| Rate::new(self.config.max_upgrades_per_sec)).allow();
        if buckets.len() > UPGRADE_BUCKETS {
            buckets.retain(|_, bucket| !bucket.unused_for(now, UPGRADE_BUCKET_IDLE));
        }
        allowed
    }

    /// Count a refused connection. A single refusal is a debug line at the refusal site; this is
    /// the deployment-wide number an operator can watch.
    fn note_refused(&self, ip: IpAddr, reason: &'static str) {
        let refused = self.refused.fetch_add(1, Ordering::Relaxed) + 1;
        if refused % 1000 == 0 {
            tracing::warn!(%ip, reason, refused, "relay: connections refused");
        }
    }

    /// Charge `bytes` to the deployment's budget. A refused datagram is dropped, never a reason to
    /// close the session: one abuser must not end another player's race.
    fn take_bandwidth(&self, bytes: usize) -> bool {
        self.bandwidth.lock().unwrap_or_else(|e| e.into_inner()).take(bytes)
    }

    fn note_dropped(&self, ip: IpAddr) {
        let dropped = self.dropped.fetch_add(1, Ordering::Relaxed) + 1;
        if dropped % 1000 == 0 {
            tracing::warn!(%ip, dropped, "relay: bandwidth budget exhausted; datagrams dropped");
        }
    }
}

/// Why a connection was refused. All of them answer `429`, but the log names the counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admission {
    Global,
    PerIp,
    PerTarget,
}

impl Admission {
    /// The counter's name in the refusal log.
    fn name(self) -> &'static str {
        match self {
            Admission::Global => "global slots",
            Admission::PerIp => "per-ip slots",
            Admission::PerTarget => "per-target slots",
        }
    }
}

/// A token bucket in bytes shared by every connection. The burst is one second of the configured
/// rate, which keeps the game's own bursts intact while bounding the deployment's egress.
struct Bandwidth {
    rate: f64,
    tokens: f64,
    window: Instant,
}

impl Bandwidth {
    fn new(bytes_per_sec: u64) -> Self {
        let rate = bytes_per_sec as f64;
        Self { rate, tokens: rate, window: Instant::now() }
    }

    fn take(&mut self, bytes: usize) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens + now.duration_since(self.window).as_secs_f64() * self.rate).min(self.rate);
        self.window = now;
        let need = bytes as f64;
        if self.tokens >= need {
            self.tokens -= need;
            true
        } else {
            false
        }
    }
}

/// A live connection's slot in the three counters. Dropping it is the decrement, on every path.
struct ConnGuard {
    state: Arc<AppState>,
    ip: IpAddr,
    target: SocketAddr,
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.state.active.fetch_sub(1, Ordering::SeqCst);
        self.state.release_ip(self.ip);
        if let Ok(mut counts) = self.state.per_target.lock() {
            if let Some(count) = counts.get_mut(&self.target) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    counts.remove(&self.target);
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct RelayQuery {
    target: Option<String>,
}

/// The routes, with their state. Exposed for the integration tests.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/relay", get(upgrade))
        .with_state(state)
}

async fn health() -> &'static str {
    "ok\n"
}

async fn upgrade(
    State(state): State<Arc<AppState>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<RelayQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !state.config.allowed_origin(origin) {
        tracing::debug!(?origin, "relay: origin refused");
        return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
    }
    let Some(text) = query.target.as_deref() else {
        return (StatusCode::BAD_REQUEST, "missing target").into_response();
    };
    let target = match TargetPolicy::parse(text) {
        Ok(target) => target,
        Err(error) => return (StatusCode::BAD_REQUEST, error).into_response(),
    };
    if let Err(error) = state.policy.validate(target) {
        tracing::debug!(%target, error, "relay: target refused");
        return (StatusCode::BAD_REQUEST, error).into_response();
    }
    let ip = client_ip(&headers, peer.ip(), &state.config.trusted_proxies);
    if !state.admit_upgrade(ip) {
        state.note_refused(ip, "upgrade rate");
        tracing::debug!(%ip, "relay: upgrade rate limit");
        return (StatusCode::TOO_MANY_REQUESTS, "too many connection attempts").into_response();
    }
    let guard = match state.acquire(ip, target) {
        Ok(guard) => guard,
        Err(reason) => {
            state.note_refused(ip, reason.name());
            tracing::debug!(%ip, %target, ?reason, "relay: connection limit reached");
            return (StatusCode::TOO_MANY_REQUESTS, "too many connections").into_response();
        }
    };
    tracing::debug!(%target, %ip, "relay: WebSocket opened");
    ws.max_message_size(MAX_FRAME).max_frame_size(MAX_FRAME).on_upgrade(move |socket| session(socket, state, target, ip, guard))
}

/// The browser's address behind the reverse proxy.
///
/// The relay is reachable only through the proxy, so its peer is a private or loopback address and
/// the forwarded header is believed. A public peer means the relay is exposed directly; then no
/// header can be trusted and the peer is the client. The chain is read right to left, because every
/// entry a client writes itself ends up on the left.
fn client_ip(headers: &HeaderMap, peer: IpAddr, trusted: &[Cidr]) -> IpAddr {
    if !is_trusted_proxy(peer, trusted) {
        return peer;
    }
    let mut chain: Vec<IpAddr> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    if chain.is_empty()
        && let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse().ok())
    {
        chain.push(ip);
    }
    chain
        .iter()
        .rev()
        .find(|ip| !is_trusted_proxy(**ip, trusted))
        .or_else(|| chain.first())
        .copied()
        .unwrap_or(peer)
}

/// A trusted proxy is this machine or a configured network. Loopback is always local, which keeps
/// the development server working without configuration.
fn is_trusted_proxy(ip: IpAddr, trusted: &[Cidr]) -> bool {
    if ip.is_loopback() {
        return true;
    }
    match ip {
        IpAddr::V4(v4) => trusted.iter().any(|cidr| cidr.contains(v4)),
        IpAddr::V6(_) => false,
    }
}

async fn session(mut ws: WebSocket, state: Arc<AppState>, target: SocketAddr, ip: IpAddr, _guard: ConnGuard) {
    let udp = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(socket) => socket,
        Err(error) => {
            tracing::warn!(error = %error, "relay: cannot bind the UDP socket");
            close(&mut ws, 1011, "relay cannot open UDP").await;
            return;
        }
    };
    if let Err(error) = udp.connect(target).await {
        tracing::debug!(%target, error = %error, "relay: UDP connect failed");
        close(&mut ws, 1011, "relay cannot reach the host").await;
        return;
    }
    if let Ok(local) = udp.local_addr()
        && let Some(frame) = relay::encode_endpoint(local)
        && ws.send(Message::Binary(Bytes::copy_from_slice(&frame))).await.is_err()
    {
        return;
    }

    let mut shutdown = state.shutdown.subscribe();
    let idle = state.config.idle;
    let mut up = Rate::new(state.config.max_pps);
    let mut down = Rate::new(state.config.max_pps);
    // Only the game's own datagrams count as activity. A ping or a text frame costs a read but is
    // not evidence of a live game, so it must not keep a connection slot forever.
    let mut deadline = tokio::time::Instant::now() + idle;
    let mut buf = vec![0u8; MAX_DATAGRAM];
    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                close(&mut ws, 1001, "relay shutting down").await;
                break;
            }
            message = ws.recv() => match message {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(message)) => {
                    // Every frame spends one token, binary or not: a ping is answered with a pong,
                    // and an ignored text frame still costs the relay a read.
                    if !up.allow() {
                        tracing::debug!(%ip, "relay: client rate limit");
                        close(&mut ws, 1008, "rate limit").await;
                        break;
                    }
                    match message {
                        Message::Binary(bytes) => {
                            let datagram = match relay::decode_client(&bytes) {
                                Ok(datagram) => datagram,
                                Err(error) => {
                                    tracing::debug!(%ip, %error, "relay: bad frame");
                                    close(&mut ws, 1008, "bad frame").await;
                                    break;
                                }
                            };
                            deadline = tokio::time::Instant::now() + idle;
                            if !state.take_bandwidth(datagram.len()) {
                                state.note_dropped(ip);
                                continue;
                            }
                            if let Err(error) = udp.send(datagram).await {
                                tracing::debug!(%target, error = %error, "relay: UDP send failed");
                                close(&mut ws, 1011, "host unreachable").await;
                                break;
                            }
                        }
                        Message::Ping(payload) => {
                            // The pong is egress too, so it comes out of the same budget.
                            if state.take_bandwidth(payload.len()) {
                                let _ = ws.send(Message::Pong(payload)).await;
                            } else {
                                state.note_dropped(ip);
                            }
                        }
                        _ => {} // text and pong carry nothing for the game
                    }
                }
                Some(Err(error)) => {
                    tracing::debug!(%ip, %error, "relay: WebSocket error");
                    break;
                }
            },
            received = udp.recv(&mut buf) => match received {
                Ok(n) => {
                    deadline = tokio::time::Instant::now() + idle;
                    if !down.allow() {
                        tracing::debug!(%ip, "relay: host rate limit");
                        close(&mut ws, 1008, "rate limit").await;
                        break;
                    }
                    if !state.take_bandwidth(n) {
                        state.note_dropped(ip);
                        continue;
                    }
                    let frame = match relay::encode_server(target, &buf[..n]) {
                        Ok(frame) => frame,
                        Err(error) => {
                            tracing::debug!(%target, %error, "relay: host datagram refused");
                            continue;
                        }
                    };
                    if ws.send(Message::Binary(Bytes::from(frame))).await.is_err() {
                        break;
                    }
                }
                Err(error) => {
                    tracing::debug!(%target, error = %error, "relay: UDP receive failed");
                    close(&mut ws, 1011, "host unreachable").await;
                    break;
                }
            },
            _ = tokio::time::sleep_until(deadline) => {
                close(&mut ws, 1000, "idle").await;
                break;
            }
        }
    }
    tracing::debug!(%target, %ip, "relay: WebSocket closed");
}

async fn close(ws: &mut WebSocket, code: u16, reason: &'static str) {
    let _ = ws.send(Message::Close(Some(CloseFrame { code, reason: reason.into() }))).await;
}

/// A one-second burst of events — datagrams per direction, or upgrade attempts per address —
/// refilled at `limit` per second. A game client sends tens of datagrams per second; the limit
/// leaves room for the original's bursts without letting one connection flood a host. A bucket
/// rather than a fixed window, so two windows cannot be spent at the same instant.
struct Rate {
    capacity: f64,
    tokens: f64,
    per_second: f64,
    window: Instant,
}

impl Rate {
    fn new(limit: u32) -> Self {
        let limit = f64::from(limit);
        Self { capacity: limit, tokens: limit, per_second: limit, window: Instant::now() }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens + now.duration_since(self.window).as_secs_f64() * self.per_second).min(self.capacity);
        self.window = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Whether the bucket can be forgotten from the per-address map: the full second of burst has
    /// long since refilled, so forgetting it cannot let an address spend more than a fresh one.
    fn unused_for(&self, now: Instant, idle: Duration) -> bool {
        now.duration_since(self.window) > idle
    }
}

/// Bind the configured address and serve until SIGINT/SIGTERM. A signal closes live sessions before
/// the process exits, so the browser sees a clean close instead of a dead TCP connection.
pub async fn run(config: Config) -> anyhow::Result<()> {
    tracing::info!(
        origins = ?config.allowed_origins,
        ports = ?config.allowed_ports,
        cidrs = ?config.allow_cidrs,
        loopback = config.allow_loopback,
        private = config.allow_private,
        proxies = ?config.trusted_proxies,
        max_connections = config.max_connections,
        max_connections_per_target = config.max_connections_per_target,
        max_pps = config.max_pps,
        max_bytes_per_sec = config.max_bytes_per_sec,
        "relay policy"
    );
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    let local = listener.local_addr()?;
    tracing::info!(%local, "relay listening");
    let state = Arc::new(AppState::new(config));
    let app = router(state.clone());
    let (stop, mut stopped) = watch::channel(false);
    let signalling = state.clone();
    tokio::spawn(async move {
        wait_for_signal().await;
        let _ = signalling.shutdown.send(());
        let _ = stop.send(true);
    });
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async move {
            let _ = stopped.changed().await;
        })
        .await?;
    Ok(())
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        let mut term = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(_) => return,
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::time::Duration;

    fn config() -> Config {
        Config { allowed_origins: Vec::new(), allow_loopback: true, ..Config::default() }
    }

    #[test]
    fn the_rate_refills_at_the_configured_pace() {
        let mut r = Rate::new(1);
        assert!(r.allow());
        assert!(!r.allow());
        r.window = Instant::now() - Duration::from_secs(2);
        assert!(r.allow());
    }

    #[test]
    fn the_bandwidth_budget_holds_a_second_of_burst() {
        let mut b = Bandwidth::new(1000);
        assert!(b.take(600));
        assert!(b.take(400));
        assert!(!b.take(1));
        b.window = Instant::now() - Duration::from_secs(1);
        assert!(b.take(500));
    }

    #[test]
    fn a_forwarded_header_needs_a_trusted_peer() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));
        let trusted = &Config::default().trusted_proxies[..];
        let client: IpAddr = "1.2.3.4".parse().unwrap();
        // A public peer is a direct client: the header it writes is ignored.
        assert_eq!(client_ip(&headers, "8.8.8.8".parse().unwrap(), trusted), "8.8.8.8".parse::<IpAddr>().unwrap());
        // The compose network is a proxy, so its header names the browser.
        assert_eq!(client_ip(&headers, "172.17.0.1".parse().unwrap(), trusted), client);
    }

    #[test]
    fn the_forwarded_chain_is_read_from_the_right() {
        let mut headers = HeaderMap::new();
        // A client-written entry sits on the left; the proxy appended its own on the right.
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.9, 1.2.3.4, 172.17.0.1"));
        let trusted = &Config::default().trusted_proxies[..];
        assert_eq!(client_ip(&headers, "172.17.0.1".parse().unwrap(), trusted), "1.2.3.4".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn one_address_cannot_flood_upgrades() {
        let mut c = config();
        c.max_upgrades_per_sec = 1;
        let state = Arc::new(AppState::new(c));
        let ip: IpAddr = "198.51.100.1".parse().unwrap();
        assert!(state.admit_upgrade(ip));
        assert!(!state.admit_upgrade(ip));
        // Another address has its own bucket.
        assert!(state.admit_upgrade("198.51.100.2".parse().unwrap()));
    }

    #[test]
    fn a_quiet_upgrade_bucket_is_forgotten() {
        let mut r = Rate::new(1);
        assert!(r.allow());
        r.window = Instant::now() - Duration::from_secs(700);
        assert!(r.unused_for(Instant::now(), UPGRADE_BUCKET_IDLE));
    }

    #[test]
    fn a_target_cannot_take_every_slot() {
        let mut c = config();
        c.max_connections_per_target = 1;
        let state = Arc::new(AppState::new(c));
        let target: SocketAddr = "93.184.216.34:9442".parse().unwrap();
        let first = state.acquire("198.51.100.1".parse().unwrap(), target).unwrap();
        assert!(matches!(state.acquire("198.51.100.2".parse().unwrap(), target), Err(Admission::PerTarget)));
        drop(first);
        assert!(state.acquire("198.51.100.2".parse().unwrap(), target).is_ok());
    }

    #[test]
    fn one_client_cannot_take_every_slot() {
        let mut c = config();
        c.max_connections_per_ip = 1;
        let state = Arc::new(AppState::new(c));
        let a: SocketAddr = "93.184.216.34:9442".parse().unwrap();
        let b: SocketAddr = "93.184.216.35:9442".parse().unwrap();
        let first = state.acquire("198.51.100.1".parse().unwrap(), a).unwrap();
        assert!(matches!(state.acquire("198.51.100.1".parse().unwrap(), b), Err(Admission::PerIp)));
        drop(first);
    }

    #[test]
    fn dropping_a_guard_returns_every_slot() {
        let state = Arc::new(AppState::new(config()));
        let ip: IpAddr = "198.51.100.1".parse().unwrap();
        let target: SocketAddr = "93.184.216.34:9442".parse().unwrap();
        drop(state.acquire(ip, target).unwrap());
        assert_eq!(state.active.load(Ordering::SeqCst), 0);
        assert!(state.per_ip.lock().unwrap().is_empty());
        assert!(state.per_target.lock().unwrap().is_empty());
    }
}
