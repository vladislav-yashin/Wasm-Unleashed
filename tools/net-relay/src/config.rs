//! Configuration for the browser relay, from the command line and `RELAY_*` variables.
//!
//! The defaults are deliberately narrow: port 9442 only, global unicast destinations only,
//! localhost origins, and forwarded addresses trusted only from private and loopback peers — the
//! networks a reverse proxy sits on. A public deployment names its own origin and its proxy network
//! in `RELAY_ALLOWED_ORIGINS` and `RELAY_TRUSTED_PROXIES`; see
//! `packaging/web/deploy/deploy.env.example`.

use crate::target::{Cidr, TargetPolicy};
use std::net::SocketAddr;
use std::time::Duration;

/// What the relay accepts. Everything a connection is allowed to do is fixed here at start-up.
#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    /// Exact `Origin` values allowed on the WebSocket upgrade. Empty allows any origin, which is
    /// only sensible for a private test; a deployment sets its site.
    pub allowed_origins: Vec<String>,
    /// The UDP ports a browser may reach. The game's own port is 9442.
    pub allowed_ports: Vec<u16>,
    /// Optional IPv4 CIDR allowlist for destinations. When set it replaces the default public-only
    /// rule, so the operator names exactly which networks a browser may reach.
    pub allow_cidrs: Vec<Cidr>,
    /// Allow loopback destinations. Tests set it; the server refuses it by default so a browser
    /// cannot use the relay to reach the relay's own machine.
    pub allow_loopback: bool,
    /// Allow private and other non-public destinations. LAN development sets it; a public relay
    /// keeps it off so it cannot be used to probe the network it runs on.
    pub allow_private: bool,
    /// The networks whose `X-Forwarded-For` the relay believes. Loopback is always trusted, because
    /// a local proxy is the only thing that can connect from it.
    pub trusted_proxies: Vec<Cidr>,
    pub max_connections: usize,
    pub max_connections_per_ip: usize,
    /// Concurrent WebSockets bound to one destination. Bounds what many client addresses can do to
    /// a single host.
    pub max_connections_per_target: usize,
    /// Datagrams per second in each direction of one connection.
    pub max_pps: u32,
    /// WebSocket upgrades one client address may start per second. The edge bounds the same
    /// attempt rate; repeating it here keeps a directly exposed relay from accepting an unbounded
    /// handshake flood.
    pub max_upgrades_per_sec: u32,
    /// Bytes per second across every connection in one direction. The per-connection limit bounds
    /// one session; this bounds the deployment's bill.
    pub max_bytes_per_sec: u64,
    /// How long a connection may be silent, both directions included.
    pub idle: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:8090".parse().expect("valid default listen address"),
            allowed_origins: vec!["http://127.0.0.1:8080".into(), "http://localhost:8080".into()],
            allowed_ports: vec![nfs_net::PORT],
            allow_cidrs: Vec::new(),
            allow_loopback: false,
            allow_private: false,
            // A reverse proxy is a private peer. A public peer means the relay is exposed directly,
            // and then its forwarded headers are ignored instead of believed.
            trusted_proxies: ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]
                .iter()
                .map(|cidr| Cidr::parse(cidr).expect("valid default proxy network"))
                .collect(),
            max_connections: 256,
            max_connections_per_ip: 16,
            max_connections_per_target: 8,
            max_pps: 400,
            max_upgrades_per_sec: 10,
            max_bytes_per_sec: 8 * 1024 * 1024,
            idle: Duration::from_secs(120),
        }
    }
}

impl Config {
    /// The address policy this configuration describes.
    pub fn policy(&self) -> TargetPolicy {
        TargetPolicy {
            allowed_ports: self.allowed_ports.clone(),
            allow_cidrs: self.allow_cidrs.clone(),
            allow_loopback: self.allow_loopback,
            allow_private: self.allow_private,
        }
    }

    pub fn allowed_origin(&self, origin: Option<&str>) -> bool {
        if self.allowed_origins.is_empty() {
            return true;
        }
        origin.is_some_and(|o| self.allowed_origins.iter().any(|allowed| allowed.eq_ignore_ascii_case(o)))
    }
}

/// Read `--flag value` pairs (and the matching `RELAY_*` variables). Unknown flags are an error,
/// because a typo in a security setting should stop the relay rather than silently widen it.
pub fn from_args(args: impl IntoIterator<Item = String>) -> anyhow::Result<Config> {
    let mut config = Config::default();
    // Environment first, command line after it: an explicit flag wins over the service file.
    if let Ok(v) = std::env::var("RELAY_LISTEN") {
        config.listen = v.parse()?;
    }
    if let Ok(v) = std::env::var("RELAY_ALLOWED_ORIGINS") {
        config.allowed_origins = csv(&v);
    }
    if let Ok(v) = std::env::var("RELAY_PORTS") {
        config.allowed_ports = ports(&v)?;
    }
    if let Ok(v) = std::env::var("RELAY_ALLOW_CIDRS") {
        config.allow_cidrs = cidrs(&v)?;
    }
    if let Ok(v) = std::env::var("RELAY_MAX_CONNECTIONS") {
        config.max_connections = v.parse()?;
    }
    if let Ok(v) = std::env::var("RELAY_MAX_CONNECTIONS_PER_IP") {
        config.max_connections_per_ip = v.parse()?;
    }
    if let Ok(v) = std::env::var("RELAY_MAX_CONNECTIONS_PER_TARGET") {
        config.max_connections_per_target = v.parse()?;
    }
    if let Ok(v) = std::env::var("RELAY_MAX_PPS") {
        config.max_pps = v.parse()?;
    }
    if let Ok(v) = std::env::var("RELAY_MAX_UPGRADES_PER_SEC") {
        config.max_upgrades_per_sec = v.parse()?;
    }
    if let Ok(v) = std::env::var("RELAY_MAX_BYTES_PER_SEC") {
        config.max_bytes_per_sec = v.parse()?;
    }
    if let Ok(v) = std::env::var("RELAY_IDLE_TIMEOUT") {
        config.idle = Duration::from_secs(v.parse()?);
    }
    if std::env::var("RELAY_ALLOW_LOOPBACK").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true")) {
        config.allow_loopback = true;
    }
    if std::env::var("RELAY_ALLOW_PRIVATE").is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true")) {
        config.allow_private = true;
    }
    if let Ok(v) = std::env::var("RELAY_TRUSTED_PROXIES") {
        config.trusted_proxies = cidrs(&v)?;
    }
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| -> anyhow::Result<String> {
            args.next().ok_or_else(|| anyhow::anyhow!("{name} needs a value"))
        };
        match arg.as_str() {
            "--listen" => config.listen = value("--listen")?.parse()?,
            "--allowed-origins" => config.allowed_origins = csv(&value("--allowed-origins")?),
            "--allowed-ports" => config.allowed_ports = ports(&value("--allowed-ports")?)?,
            "--allow-cidrs" => config.allow_cidrs = cidrs(&value("--allow-cidrs")?)?,
            "--allow-loopback" => config.allow_loopback = true,
            "--allow-private" => config.allow_private = true,
            "--trusted-proxies" => config.trusted_proxies = cidrs(&value("--trusted-proxies")?)?,
            "--max-connections" => config.max_connections = value("--max-connections")?.parse()?,
            "--max-connections-per-ip" => config.max_connections_per_ip = value("--max-connections-per-ip")?.parse()?,
            "--max-connections-per-target" => config.max_connections_per_target = value("--max-connections-per-target")?.parse()?,
            "--max-pps" => config.max_pps = value("--max-pps")?.parse()?,
            "--max-upgrades-per-sec" => config.max_upgrades_per_sec = value("--max-upgrades-per-sec")?.parse()?,
            "--max-bytes-per-sec" => config.max_bytes_per_sec = value("--max-bytes-per-sec")?.parse()?,
            "--idle-timeout" => config.idle = Duration::from_secs(value("--idle-timeout")?.parse()?),
            "--help" | "-h" => {
                println!("{}", HELP);
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument {other:?}; see --help"),
        }
    }
    if config.allowed_ports.is_empty() {
        anyhow::bail!("the relay needs at least one UDP port in --allowed-ports");
    }
    if config.max_connections == 0 || config.max_connections_per_ip == 0 || config.max_connections_per_target == 0 || config.max_pps == 0 || config.max_upgrades_per_sec == 0 || config.max_bytes_per_sec == 0 {
        anyhow::bail!("connection, rate and bandwidth limits must be greater than zero");
    }
    if config.allowed_origins.is_empty() {
        tracing::warn!("no relay origins configured: any WebSocket origin is accepted");
    }
    Ok(config)
}

fn csv(value: &str) -> Vec<String> {
    value.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

fn ports(value: &str) -> anyhow::Result<Vec<u16>> {
    let mut out = Vec::new();
    for part in csv(value) {
        let port: u16 = part.parse()?;
        if port == 0 {
            anyhow::bail!("port 0 is not a destination port");
        }
        if !out.contains(&port) {
            out.push(port);
        }
    }
    Ok(out)
}

fn cidrs(value: &str) -> anyhow::Result<Vec<Cidr>> {
    csv(value).iter().map(|s| Cidr::parse(s)).collect()
}

pub const HELP: &str = "\
nfs-relay — WebSocket to UDP relay for browser network games

Usage: nfs-relay [options]

  --listen ADDR                  WebSocket/HTTP bind address (default 0.0.0.0:8090)
  --allowed-origins LIST         comma-separated Origin values; empty accepts any
  --allowed-ports LIST           comma-separated UDP destination ports (default 9442)
  --allow-cidrs LIST             comma-separated IPv4 CIDRs; replaces the public-only rule
  --allow-loopback               allow loopback destinations (tests only)
  --allow-private                allow private and special-purpose destinations (LAN only)
  --trusted-proxies LIST         comma-separated IPv4 CIDRs whose X-Forwarded-For is believed
                                 (loopback is always trusted; default 10/8,172.16/12,192.168/16)
  --max-connections N            total concurrent WebSockets (default 256)
  --max-connections-per-ip N     concurrent WebSockets per client IP (default 16)
  --max-connections-per-target N concurrent WebSockets per destination (default 8)
  --max-pps N                    datagrams per second per direction (default 400)
  --max-upgrades-per-sec N       WebSocket upgrades per client address per second (default 10)
  --max-bytes-per-sec N          bytes per second across all connections in one direction
                                 (default 8388608)
  --idle-timeout SECS            close a silent connection after this long (default 120)
  --help                         print this text

Environment: RELAY_LISTEN, RELAY_ALLOWED_ORIGINS, RELAY_PORTS, RELAY_ALLOW_CIDRS,
RELAY_ALLOW_LOOPBACK, RELAY_ALLOW_PRIVATE, RELAY_TRUSTED_PROXIES, RELAY_MAX_CONNECTIONS,
RELAY_MAX_CONNECTIONS_PER_IP, RELAY_MAX_CONNECTIONS_PER_TARGET, RELAY_MAX_PPS,
RELAY_MAX_UPGRADES_PER_SEC, RELAY_MAX_BYTES_PER_SEC, RELAY_IDLE_TIMEOUT
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_narrow_ones() {
        let c = Config::default();
        assert_eq!(c.allowed_ports, vec![nfs_net::PORT]);
        assert!(c.allow_cidrs.is_empty());
        assert!(!c.allow_loopback);
        assert!(!c.allow_private);
        assert_eq!(c.max_connections_per_target, 8);
        assert_eq!(c.max_upgrades_per_sec, 10);
        assert_eq!(c.max_bytes_per_sec, 8 * 1024 * 1024);
        assert!(c.trusted_proxies.iter().any(|cidr| cidr.contains("172.17.0.1".parse().unwrap())));
        assert!(!c.policy().allow_private);
    }

    #[test]
    fn a_public_relay_reaches_global_unicast_only() {
        let policy = Config::default().policy();
        assert!(policy.validate("93.184.216.34:9442".parse().unwrap()).is_ok());
        for addr in ["10.1.2.3:9442", "172.16.0.1:9442", "192.168.1.5:9442", "100.64.0.1:9442", "127.0.0.1:9442"] {
            assert!(policy.validate(addr.parse().unwrap()).is_err(), "{addr} should be refused");
        }
    }

    #[test]
    fn allow_private_is_an_opt_in() {
        let config = from_args(["--allow-private".to_string()]).unwrap();
        assert!(config.policy().allow_private);
        assert!(config.policy().validate("192.168.1.5:9442".parse().unwrap()).is_ok());
    }

    #[test]
    fn the_new_limits_come_from_flags() {
        let config = from_args([
            "--trusted-proxies".to_string(),
            "172.20.0.0/16".to_string(),
            "--max-connections-per-target".to_string(),
            "3".to_string(),
            "--max-upgrades-per-sec".to_string(),
            "5".to_string(),
            "--max-bytes-per-sec".to_string(),
            "1024".to_string(),
        ])
        .unwrap();
        assert_eq!(config.trusted_proxies.len(), 1);
        assert_eq!(config.max_connections_per_target, 3);
        assert_eq!(config.max_upgrades_per_sec, 5);
        assert_eq!(config.max_bytes_per_sec, 1024);
    }

    #[test]
    fn a_typo_still_stops_the_relay() {
        assert!(from_args(["--allow-privat".to_string()]).is_err());
        assert!(from_args(["--max-bytes-per-sec".to_string(), "0".to_string()]).is_err());
        assert!(from_args(["--trusted-proxies".to_string(), "10.0.0.0/33".to_string()]).is_err());
    }

    #[test]
    fn an_empty_proxy_list_trusts_loopback_only() {
        let config = from_args(["--trusted-proxies".to_string(), String::new()]).unwrap();
        assert!(config.trusted_proxies.is_empty());
    }

    #[test]
    fn the_origin_list_is_exact_and_optional() {
        let mut c = Config::default();
        assert!(c.allowed_origin(Some("http://127.0.0.1:8080")));
        assert!(!c.allowed_origin(Some("http://evil.test")));
        assert!(!c.allowed_origin(None));
        c.allowed_origins.clear();
        assert!(c.allowed_origin(None));
    }
}
