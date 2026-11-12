//! Addresses for authentication (spec auth): address ranges, the client address behind
//! trusted proxies, and the key the login throttling counts by.

use std::{
    net::{IpAddr, Ipv6Addr},
    str::FromStr,
};

/// An address range (`192.168.1.0/24`, `fd00::/8`) or a single address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    network: IpAddr,
    prefix: u8,
}

/// An IPv4-mapped IPv6 address counts as its IPv4 address.
pub fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

fn bits(ip: IpAddr) -> (u128, u8) {
    match ip {
        IpAddr::V4(v4) => (u128::from(u32::from(v4)), 32),
        IpAddr::V6(v6) => (u128::from(v6), 128),
    }
}

fn mask(width: u8, prefix: u8) -> u128 {
    if prefix == 0 {
        0
    } else {
        let all = if width == 32 {
            u128::from(u32::MAX)
        } else {
            u128::MAX
        };
        all & !((1u128 << (width - prefix)) - 1)
    }
}

impl FromStr for Cidr {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (addr, prefix) = match s.trim().split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s.trim(), None),
        };
        let network = canonical(IpAddr::from_str(addr).map_err(|_| ())?);
        let (_, width) = bits(network);
        // A mapped IPv6 range (`::ffff:10.0.0.0/104`) loses the 96 mapping bits.
        let mapped = network.is_ipv4() && addr.contains(':');
        let prefix = match prefix {
            None => width,
            Some(p) => {
                let p: u8 = p.parse().map_err(|_| ())?;
                let p = if mapped {
                    p.checked_sub(96).ok_or(())?
                } else {
                    p
                };
                if p > width {
                    return Err(());
                }
                p
            }
        };
        Ok(Self { network, prefix })
    }
}

impl Cidr {
    pub fn contains(&self, ip: IpAddr) -> bool {
        let (net, width) = bits(self.network);
        let (addr, addr_width) = bits(canonical(ip));
        width == addr_width && (net ^ addr) & mask(width, self.prefix) == 0
    }
}

fn trusted(proxies: &[Cidr], ip: IpAddr) -> bool {
    proxies.iter().any(|c| c.contains(ip))
}

/// Where a request counts as coming from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    Known(IpAddr),
    /// A trusted proxy forwarded an invalid address: in no trusted network, throttled
    /// under the proxy's address.
    Unknown {
        peer: IpAddr,
    },
}

impl Client {
    /// Whether the client lies in one of the ranges; an unknown client never does.
    pub fn in_any(&self, ranges: &[Cidr]) -> bool {
        match self {
            Self::Known(ip) => ranges.iter().any(|c| c.contains(*ip)),
            Self::Unknown { .. } => false,
        }
    }

    /// What the login throttling counts the client by.
    pub fn throttle_key(&self) -> IpAddr {
        match self {
            Self::Known(ip) | Self::Unknown { peer: ip } => throttle_key(*ip),
        }
    }
}

impl std::fmt::Display for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Known(ip) => write!(f, "{ip}"),
            Self::Unknown { peer } => write!(f, "unknown via {peer}"),
        }
    }
}

/// The client of a request: from a trusted proxy the rightmost `X-Forwarded-For` entry
/// that is not a trusted proxy itself (the leftmost if all are), the peer if the header
/// is missing; an invalid entry met (from the right) before such an address makes the
/// client unknown. From any other peer the header is ignored.
pub fn client_address(peer: IpAddr, forwarded: &[&str], proxies: &[Cidr]) -> Client {
    let peer = canonical(peer);
    if !trusted(proxies, peer) {
        return Client::Known(peer);
    }
    let entries: Vec<&str> = forwarded.iter().flat_map(|h| h.split(',')).collect();
    let mut leftmost = None;
    for entry in entries.iter().rev() {
        let Ok(ip) = IpAddr::from_str(entry.trim()) else {
            return Client::Unknown { peer };
        };
        let ip = canonical(ip);
        if !trusted(proxies, ip) {
            return Client::Known(ip);
        }
        leftmost = Some(ip);
    }
    Client::Known(leftmost.unwrap_or(peer))
}

/// What the login throttling counts by: an IPv4 address, or the /64 of an IPv6 one.
pub fn throttle_key(ip: IpAddr) -> IpAddr {
    match canonical(ip) {
        IpAddr::V6(v6) => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & mask(128, 64))),
        v4 => v4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().expect("ip")
    }

    fn cidr(s: &str) -> Cidr {
        s.parse().expect("cidr")
    }

    #[test]
    fn ranges_contain_addresses() {
        let lan = cidr("192.168.1.0/24");
        assert!(lan.contains(ip("192.168.1.77")));
        assert!(!lan.contains(ip("192.168.2.1")));
        // IPv4-mapped IPv6 is the IPv4 address.
        assert!(lan.contains(ip("::ffff:192.168.1.5")));
        assert!(cidr("::ffff:192.168.1.0/120").contains(ip("192.168.1.9")));
        assert!(cidr("10.0.0.1").contains(ip("10.0.0.1")));
        assert!(!cidr("10.0.0.1").contains(ip("10.0.0.2")));
        assert!(cidr("0.0.0.0/0").contains(ip("8.8.8.8")));
        assert!(!cidr("0.0.0.0/0").contains(ip("2001:db8::1")));
        assert!(cidr("fd00::/8").contains(ip("fd12:3456::1")));
        assert!(!cidr("fd00::/8").contains(ip("fe80::1")));
        for bad in ["", "x", "10.0.0.0/33", "fd00::/129", "10.0.0.0/x", "1.2.3"] {
            assert!(bad.parse::<Cidr>().is_err(), "{bad}");
        }
    }

    #[test]
    fn client_address_behind_trusted_proxies() {
        let proxies = [cidr("10.0.0.0/8")];
        let peer = ip("10.0.0.2");
        // The rightmost entry that is not a trusted proxy.
        assert_eq!(
            client_address(peer, &["203.0.113.7, 10.0.0.9"], &proxies),
            Client::Known(ip("203.0.113.7"))
        );
        assert_eq!(
            client_address(peer, &["1.1.1.1", "203.0.113.7, 10.0.0.9"], &proxies),
            Client::Known(ip("203.0.113.7"))
        );
        // Only proxies: the leftmost.
        assert_eq!(
            client_address(peer, &["10.0.0.5, 10.0.0.9"], &proxies),
            Client::Known(ip("10.0.0.5"))
        );
        // Missing header: the peer.
        assert_eq!(client_address(peer, &[], &proxies), Client::Known(peer));
        // An invalid entry before any non-proxy address (read from the right): unknown.
        assert_eq!(
            client_address(peer, &["203.0.113.7, unknown"], &proxies),
            Client::Unknown { peer }
        );
        assert_eq!(
            client_address(peer, &["garbage, 10.0.0.9"], &proxies),
            Client::Unknown { peer }
        );
        // A valid non-proxy address right of the invalid entry counts.
        assert_eq!(
            client_address(peer, &["garbage, 203.0.113.9"], &proxies),
            Client::Known(ip("203.0.113.9"))
        );
        // From an untrusted peer the header is ignored.
        assert_eq!(
            client_address(ip("203.0.113.9"), &["192.168.1.5"], &proxies),
            Client::Known(ip("203.0.113.9"))
        );
        assert_eq!(
            client_address(ip("::ffff:203.0.113.9"), &[], &[]),
            Client::Known(ip("203.0.113.9"))
        );
    }

    #[test]
    fn throttling_counts_ipv6_by_its_64() {
        assert_eq!(
            throttle_key(ip("2001:db8:1:2:aaaa::1")),
            throttle_key(ip("2001:db8:1:2:bbbb::9"))
        );
        assert_ne!(
            throttle_key(ip("2001:db8:1:2::1")),
            throttle_key(ip("2001:db8:1:3::1"))
        );
        assert_eq!(throttle_key(ip("::ffff:192.0.2.1")), ip("192.0.2.1"));
    }
}
