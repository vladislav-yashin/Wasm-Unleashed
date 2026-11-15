//! End-to-end: a browser-shaped WebSocket client joins a real `nfs-net` host through the relay.
//!
//! The host is the same session object the game runs; the client sends the original's `'j'` and
//! expects the `'a'` the host's admit path builds. This proves the relay carries the real protocol,
//! not just bytes: the slot mask, the join reply and the source address all come from the host.

use futures_util::{SinkExt, StreamExt};
use nfs_net::comm::{self, tag, Join, JoinReply, JoinStatus};
use nfs_net::relay::{self, ServerFrame};
use nfs_net::session::{name16, name32, Event, Net};
use nfs_net::socket::{record_of, CommSocket};
use nfs_net::wire::Datagram;
use nfs_relay::config::Config;
use nfs_relay::server::{router, AppState};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn a_websocket_client_joins_a_native_host_through_the_relay() {
    const KEY: [u8; 32] = [0x5A; 32];
    // The real host: the same type the desktop game uses.
    let mut host = Net::host(
        CommSocket::bind_on("127.0.0.1:0".parse().unwrap()).unwrap(),
        name32("Relay Test"),
        KEY,
        9,
    );
    host.my_name = name16("host");
    let host_addr = host.local_addr().unwrap();
    assert_eq!(host_addr.ip().to_string(), "127.0.0.1");

    // Poll the host in a blocking task: its socket is the desktop transport.
    let stop = Arc::new(AtomicBool::new(false));
    let host_stop = stop.clone();
    let host_task = tokio::task::spawn_blocking(move || {
        let mut events = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !host_stop.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
            events.extend(host.poll());
            std::thread::sleep(Duration::from_millis(5));
        }
        events
    });

    // The relay, on a loopback port, allowed to reach the host's ephemeral port.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap();
    let config = Config {
        listen: relay_addr,
        allowed_origins: Vec::new(),
        allowed_ports: vec![host_addr.port()],
        allow_cidrs: Vec::new(),
        allow_loopback: true,
        max_connections: 8,
        max_connections_per_ip: 4,
        max_pps: 400,
        idle: Duration::from_secs(10),
        ..Config::default()
    };
    let server = tokio::spawn(async move {
        axum::serve(listener, router(Arc::new(AppState::new(config))).into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });

    // A browser-shaped client: one WebSocket per peer, the original datagrams inside.
    let url = format!("ws://{relay_addr}/relay?target={host_addr}");
    let mut request = url.into_client_request().unwrap();
    request.headers_mut().insert("Origin", HeaderValue::from_static("http://127.0.0.1:8080"));
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();

    let own = record_of("127.0.0.1:0".parse().unwrap()).unwrap();
    let to = record_of(host_addr).unwrap();
    let identity = comm::identity::encode("browser", "3.5", "3.5");
    let join = Join::original(1, to, own, &identity);
    let datagram = Datagram::encode(tag::JOIN, &join.encode()).unwrap();
    ws.send(Message::Binary(relay::encode_client(&datagram).unwrap().into())).await.unwrap();

    let mut endpoint = None;
    let mut joined = None;
    for _ in 0..20 {
        let message = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("the relay answered")
            .expect("the socket stayed open")
            .expect("a frame arrived");
        let Message::Binary(bytes) = message else { continue };
        match relay::decode_server(&bytes).unwrap() {
            ServerFrame::Endpoint(addr) => endpoint = Some(addr),
            ServerFrame::Data { from, datagram } => {
                assert_eq!(from, host_addr, "the host's datagrams name the host");
                let d = Datagram::decode(&datagram).unwrap();
                if d.tag == tag::A {
                    let reply = JoinReply::decode(&d.payload).unwrap();
                    assert_eq!(reply.status, JoinStatus::Joined(0));
                    joined = Some(reply);
                    break;
                }
            }
        }
    }
    assert!(endpoint.is_some(), "the relay sent its endpoint");
    assert!(joined.is_some(), "the host admitted the browser");

    ws.close(None).await.unwrap();
    stop.store(true, Ordering::Relaxed);
    let events = host_task.await.unwrap();
    assert!(
        events.iter().any(|e| matches!(e, Event::PeerJoined { slot: 0, .. })),
        "the host saw the browser as a peer: {events:?}"
    );
    server.abort();
}

#[tokio::test]
async fn the_relay_refuses_a_missing_origin_and_a_foreign_port() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap();
    let config = Config { listen: relay_addr, allowed_origins: vec!["https://game.test".into()], allowed_ports: vec![9442], allow_loopback: true, ..Config::default() };
    let server = tokio::spawn(async move {
        axum::serve(listener, router(Arc::new(AppState::new(config))).into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    let url = format!("ws://{relay_addr}/relay?target=127.0.0.1:9442");
    // No Origin header: the upgrade is refused before a socket is opened.
    assert!(tokio_tungstenite::connect_async(url.clone()).await.is_err());
    // A wrong port is refused by the target policy.
    let mut request = format!("ws://{relay_addr}/relay?target=127.0.0.1:9443").into_client_request().unwrap();
    request.headers_mut().insert("Origin", HeaderValue::from_static("https://game.test"));
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
    server.abort();
}

#[tokio::test]
async fn a_public_relay_refuses_a_lan_destination() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap();
    let config = Config { listen: relay_addr, allowed_origins: vec!["https://game.test".into()], ..Config::default() };
    let server = tokio::spawn(async move {
        axum::serve(listener, router(Arc::new(AppState::new(config))).into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    // 192.168.1.5 is a host on the relay's own side of the internet; the default policy is public
    // unicast only, so an ordinary deployment cannot be aimed at a LAN.
    let mut request = format!("ws://{relay_addr}/relay?target=192.168.1.5:9442").into_client_request().unwrap();
    request.headers_mut().insert("Origin", HeaderValue::from_static("https://game.test"));
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
    server.abort();
}

#[tokio::test]
async fn a_ping_flood_spends_the_packet_budget() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap();
    let config = Config {
        listen: relay_addr,
        allowed_origins: Vec::new(),
        allow_loopback: true,
        max_pps: 2,
        idle: Duration::from_secs(10),
        ..Config::default()
    };
    let server = tokio::spawn(async move {
        axum::serve(listener, router(Arc::new(AppState::new(config))).into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    let url = format!("ws://{relay_addr}/relay?target=127.0.0.1:9442");
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    // The endpoint frame is not the client's traffic, so the two allowed pings come after it.
    for _ in 0..3 {
        ws.send(Message::Ping(b"keep".to_vec().into())).await.unwrap();
    }
    let mut closed = None;
    for _ in 0..10 {
        let message = tokio::time::timeout(Duration::from_secs(2), ws.next())
            .await
            .expect("the relay answered")
            .expect("the socket stayed open")
            .expect("a frame arrived");
        if let Message::Close(Some(frame)) = message {
            closed = Some(frame);
            break;
        }
    }
    let frame = closed.expect("the ping flood was closed");
    assert_eq!(u16::from(frame.code), 1008, "the close code is a policy violation");
    server.abort();
}

#[tokio::test]
async fn a_ping_does_not_keep_a_silent_session_alive() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap();
    let config = Config {
        listen: relay_addr,
        allowed_origins: Vec::new(),
        allow_loopback: true,
        max_pps: 100,
        idle: Duration::from_secs(1),
        ..Config::default()
    };
    let server = tokio::spawn(async move {
        axum::serve(listener, router(Arc::new(AppState::new(config))).into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    let url = format!("ws://{relay_addr}/relay?target=127.0.0.1:9442");
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    let mut ping = tokio::time::interval(Duration::from_millis(200));
    let give_up = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        tokio::select! {
            message = ws.next() => match message.expect("the socket stayed open").expect("a frame arrived") {
                Message::Close(Some(frame)) => {
                    assert_eq!(u16::from(frame.code), 1000, "the idle close is normal");
                    let reason: &str = frame.reason.as_ref();
                    assert_eq!(reason, "idle");
                    break;
                }
                _ => {}
            },
            _ = ping.tick() => {
                ws.send(Message::Ping(b"keep".to_vec().into())).await.unwrap();
            }
            _ = tokio::time::sleep_until(give_up) => panic!("pings kept the session alive"),
        }
    }
    server.abort();
}

#[tokio::test]
async fn one_address_cannot_flood_upgrades() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = listener.local_addr().unwrap();
    let config = Config {
        listen: relay_addr,
        allowed_origins: Vec::new(),
        allow_loopback: true,
        max_upgrades_per_sec: 1,
        ..Config::default()
    };
    let server = tokio::spawn(async move {
        axum::serve(listener, router(Arc::new(AppState::new(config))).into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    let url = format!("ws://{relay_addr}/relay?target=127.0.0.1:9442");
    let first = tokio_tungstenite::connect_async(url.clone()).await.expect("the first upgrade is allowed");
    // The bucket holds one second of attempts, so an immediate retry is over the limit.
    match tokio_tungstenite::connect_async(url).await {
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            assert_eq!(response.status(), 429, "the retry is refused, not dropped");
        }
        other => panic!("the second upgrade should be refused, got {other:?}"),
    }
    drop(first);
    server.abort();
}
