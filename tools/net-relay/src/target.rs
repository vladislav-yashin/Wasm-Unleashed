//! Where the relay may send UDP.
//!
//! A relay is an obvious abuse target if it forwards anywhere, so this is an allow-policy rather
//! than a blocklist: the destination must be IPv4, use one of the configured ports, and be global
//! unicast — the only addresses a public game host has. An explicit CIDR allowlist or
//! `allow_private` opens private space for a LAN deployment; loopback needs its own switch and is
//! never enabled by the server itself.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// One IPv4 network (`a.b.c.d/prefix`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    network: u32,
    mask: u32,
}

impl Cidr {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let (addr, prefix) = text.split_once('/').ok_or_else(|| anyhow::anyhow!("{text:?} is not an IPv4 CIDR"))?;
        let addr: Ipv4Addr = addr.parse()?;
        let prefix: u32 = prefix.parse()?;
        if prefix > 32 {
            anyhow::bail!("{text:?} has a prefix longer than 32");
        }
        let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
        Ok(Self { network: u32::from(addr) & mask, mask })
    }

    pub fn contains(self, addr: Ipv4Addr) -> bool {
        u32::from(addr) & self.mask == self.network
    }
}

/// The rules [`crate::server`] applies before it opens a UDP socket.
#[derive(Debug, Clone, Default)]
pub struct TargetPolicy {
    pub allowed_ports: Vec<u16>,
    /// When set, the destination must fall inside it. The operator named these destinations, so the
    /// list replaces the default public-only rule. Loopback still needs [`Self::allow_loopback`].
    pub allow_cidrs: Vec<Cidr>,
    /// Tests only. The server never enables this by itself, so a public relay cannot be aimed at
    /// its own machine or another local service.
    pub allow_loopback: bool,
    /// LAN development only. Without it a public relay refuses RFC1918, carrier-grade NAT and the
    /// other non-public ranges, so it cannot be used to probe the network it runs on.
    pub allow_private: bool,
}

impl TargetPolicy {
    pub fn validate(&self, target: SocketAddr) -> Result<SocketAddr, String> {
        let SocketAddr::V4(v4) = target else {
            return Err("the game protocol is IPv4 only".into());
        };
        let ip = *v4.ip();
        if v4.port() == 0 {
            return Err("port 0 is not a destination".into());
        }
        if !self.allowed_ports.is_empty() && !self.allowed_ports.contains(&v4.port()) {
            return Err(format!("port {} is not allowed (the game uses {})", v4.port(), self.allowed_ports.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")));
        }
        // Never a unicast destination, whatever the flags say.
        if ip.is_unspecified() || ip.octets()[0] == 0 {
            return Err("the unspecified network is not a destination".into());
        }
        if ip.is_broadcast() || ip.is_multicast() || ip.octets()[0] >= 240 {
            return Err("broadcast, multicast and reserved addresses are not destinations".into());
        }
        // Loopback keeps its own switch even when an allowlist names it: reaching the relay's own
        // machine is never an accident, and `0.0.0.0/0` must not turn it on.
        if ip.is_loopback() {
            return if self.allow_loopback { Ok(target) } else { Err("loopback destinations are not allowed".into()) };
        }
        // An explicit allowlist is the operator's decision and replaces the public-only rule below.
        if !self.allow_cidrs.is_empty() {
            if self.allow_cidrs.iter().any(|cidr| cidr.contains(ip)) {
                return Ok(target);
            }
            return Err(format!("{ip} is outside the configured allowlist"));
        }
        if self.allow_private {
            return Ok(target);
        }
        if !is_global_unicast(ip) {
            return Err(format!("{ip} is a private or special-purpose address; a public relay reaches global unicast only"));
        }
        Ok(target)
    }

    pub fn parse(text: &str) -> Result<SocketAddr, String> {
        let target: SocketAddr = text.parse().map_err(|_| format!("{text:?} is not an IPv4 address and port"))?;
        match target {
            SocketAddr::V4(v4) if v4.ip().is_unspecified() => Err(format!("{text:?} has no address")),
            SocketAddr::V4(_) => Ok(target),
            SocketAddr::V6(_) => Err("the game protocol is IPv4 only".into()),
        }
    }
}

/// The loopback address a test uses.
pub fn test_target(port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
}

/// Whether `ip` is global unicast: an address that can name a game host on the public internet.
///
/// `Ipv4Addr::is_global` is still unstable, so the non-public blocks are listed explicitly.
fn is_global_unicast(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || o[0] == 0
        || o[0] >= 240
        // 100.64.0.0/10 — shared address space (carrier-grade NAT)
        || (o[0] == 100 && (o[1] & 0xC0) == 64)
        // 192.0.0.0/24 — IETF protocol assignments
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        // 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24 — documentation
        || (o[0] == 192 && o[1] == 0 && o[2] == 2)
        || (o[0] == 198 && o[1] == 51 && o[2] == 100)
        || (o[0] == 203 && o[1] == 0 && o[2] == 113)
        // 198.18.0.0/15 — benchmarking
        || (o[0] == 198 && (o[1] & 0xFE) == 18))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> TargetPolicy {
        TargetPolicy { allowed_ports: vec![9442], allow_cidrs: vec![], allow_loopback: false, allow_private: false }
    }

    #[test]
    fn cidrs_match_their_network() {
        let c = Cidr::parse("10.1.0.0/16").unwrap();
        assert!(c.contains("10.1.2.3".parse().unwrap()));
        assert!(!c.contains("10.2.0.1".parse().unwrap()));
        assert!(Cidr::parse("0.0.0.0/0").unwrap().contains("8.8.8.8".parse().unwrap()));
        assert!(Cidr::parse("10.0.0.0/33").is_err());
    }

    #[test]
    fn the_game_port_is_the_default() {
        let p = policy();
        assert!(p.validate("93.184.216.34:9442".parse().unwrap()).is_ok());
        assert!(p.validate("93.184.216.34:80".parse().unwrap()).is_err());
    }

    #[test]
    fn private_and_special_addresses_are_refused_by_default() {
        let p = policy();
        for addr in [
            "10.1.2.3:9442",
            "172.16.0.1:9442",
            "192.168.1.5:9442",
            "100.64.0.1:9442",
            "192.0.0.1:9442",
            "192.0.2.5:9442",
            "198.18.0.1:9442",
            "198.51.100.7:9442",
            "203.0.113.9:9442",
        ] {
            assert!(p.validate(addr.parse().unwrap()).is_err(), "{addr} should be refused");
        }
    }

    #[test]
    fn allow_private_opens_the_lan_for_development() {
        let mut p = policy();
        p.allow_private = true;
        assert!(p.validate("192.168.1.5:9442".parse().unwrap()).is_ok());
        assert!(p.validate("10.1.2.3:9442".parse().unwrap()).is_ok());
        // Loopback keeps its own switch, so a LAN session cannot reach the relay's own machine.
        assert!(p.validate("127.0.0.1:9442".parse().unwrap()).is_err());
    }

    #[test]
    fn special_addresses_are_refused() {
        let mut p = policy();
        for addr in ["127.0.0.1:9442", "0.0.0.0:9442", "169.254.1.1:9442", "224.0.0.1:9442", "255.255.255.255:9442", "240.0.0.1:9442"] {
            assert!(p.validate(addr.parse().unwrap()).is_err(), "{addr} should be refused");
        }
        p.allow_loopback = true;
        assert!(p.validate("127.0.0.1:9442".parse().unwrap()).is_ok());
    }

    #[test]
    fn an_allowlist_is_a_positive_requirement() {
        let p = TargetPolicy { allowed_ports: vec![9442], allow_cidrs: vec![Cidr::parse("10.0.0.0/8").unwrap()], ..TargetPolicy::default() };
        assert!(p.validate("10.2.3.4:9442".parse().unwrap()).is_ok());
        assert!(p.validate("192.168.1.2:9442".parse().unwrap()).is_err());
    }

    #[test]
    fn an_allowlist_overrides_the_public_only_rule() {
        // The operator named the network, so a private destination is intentional.
        let p = TargetPolicy { allowed_ports: vec![9442], allow_cidrs: vec![Cidr::parse("192.168.0.0/16").unwrap()], ..TargetPolicy::default() };
        assert!(p.validate("192.168.1.5:9442".parse().unwrap()).is_ok());
        assert!(p.validate("93.184.216.34:9442".parse().unwrap()).is_err());
    }

    #[test]
    fn ipv6_is_not_part_of_this_protocol() {
        assert!(policy().validate("[::1]:9442".parse().unwrap()).is_err());
        assert!(TargetPolicy::parse("[::1]:9442").is_err());
    }
}
