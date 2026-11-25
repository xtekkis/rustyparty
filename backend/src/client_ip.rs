use std::convert::Infallible;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{Extensions, HeaderMap};

use crate::AppState;

// The address a request came from, for counting it against the limits
pub struct ClientIp(pub IpAddr);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Infallible> {
        Ok(ClientIp(client_ip(
            &parts.headers,
            &parts.extensions,
            state.behind_proxy,
        )))
    }
}

// The connection's own address, or behind a proxy like Render's, the one the proxy saw
pub fn client_ip(headers: &HeaderMap, extensions: &Extensions, behind_proxy: bool) -> IpAddr {
    if behind_proxy && let Some(ip) = cloudflare_ip(headers).or_else(|| forwarded_ip(headers)) {
        return ip;
    }
    let through_proxy = headers.contains_key("x-forwarded-for");
    if !behind_proxy && through_proxy {
        warn_about_proxy();
    }
    let peer = extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    // A proxy on this computer, like nginx in front of the server. It isn't this computer
    // playing, so it mustn't skip the limits: everyone behind it shares one count instead
    if through_proxy && peer.is_loopback() {
        return IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    }
    peer
}

// Render sits behind Cloudflare, which puts the visitor's address in CF-Connecting-IP and
// overwrites any the visitor sent, so it can't be faked. Checked on Render on 2026-11-25:
// X-Forwarded-For held the visitor, then a Cloudflare server, then Render's own hop
fn cloudflare_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("cf-connecting-ip")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_entry)
}

// Without Cloudflare: the proxy adds the address it saw to the end of X-Forwarded-For, after
// anything the visitor wrote themselves, then its own internal hops. So the rightmost public
// address is the real one
fn forwarded_ip(headers: &HeaderMap) -> Option<IpAddr> {
    let entries: Vec<&str> = headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .collect();
    entries
        .into_iter()
        .rev()
        .filter_map(parse_entry)
        .find(is_public)
}

// An address as proxies write it: maybe with a port ("1.2.3.4:5678", "[2001:db8::1]:443"),
// maybe an IPv4 address written as IPv6 ("::ffff:10.1.2.3")
fn parse_entry(entry: &str) -> Option<IpAddr> {
    let entry = entry.trim();
    let address = if let Some(bracketed) = entry.strip_prefix('[') {
        bracketed.split(']').next()?
    } else if entry.matches(':').count() == 1 {
        entry.split(':').next()?
    } else {
        entry
    };
    address.parse::<IpAddr>().ok().map(|ip| ip.to_canonical())
}

// A proxy header arrived but BEHIND_PROXY isn't set, so every player may look like the proxy
// and share one limit. Said once, so the log isn't flooded
fn warn_about_proxy() {
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        tracing::warn!(
            "Requests come through a proxy but BEHIND_PROXY is not set, so players share one limit. Set BEHIND_PROXY=true"
        );
    }
}

// Whether an address can belong to a visitor out on the internet. Private, internal and
// reserved ranges can only be a proxy's own hops
fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            let reserved = a == 0 // "this network"
                || (a == 100 && (b & 0xC0) == 64) // shared by carriers, 100.64.0.0/10
                || (a == 192 && b == 0 && c == 0) // protocol assignments, 192.0.0.0/24
                || (a == 198 && (b & 0xFE) == 18) // benchmarking, 198.18.0.0/15
                || a >= 240; // future use and broadcast, 240.0.0.0/4
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || reserved)
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.is_multicast())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn from(forwarded: &str, behind_proxy: bool) -> IpAddr {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_str(forwarded).unwrap());
        let mut extensions = Extensions::new();
        extensions.insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 5], 4000))));
        client_ip(&headers, &extensions, behind_proxy)
    }

    fn from_cloudflare(forwarded: &str, cloudflare: &str) -> IpAddr {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_str(forwarded).unwrap());
        headers.insert(
            "cf-connecting-ip",
            HeaderValue::from_str(cloudflare).unwrap(),
        );
        client_ip(&headers, &Extensions::new(), true)
    }

    #[test]
    fn on_render_cloudflares_header_is_the_player() {
        // What Render sends: the visitor, then a Cloudflare server, then Render's own hop.
        // The Cloudflare server's address is public too, so it mustn't be counted
        let ip = from_cloudflare("203.0.113.7, 172.70.1.2, 10.1.2.3", "203.0.113.7");
        assert_eq!(ip, IpAddr::from([203, 0, 113, 7]));
    }

    #[test]
    fn a_faked_forwarded_address_doesnt_beat_cloudflare() {
        let ip = from_cloudflare("1.2.3.4, 203.0.113.7, 172.70.1.2", "203.0.113.7");
        assert_eq!(ip, IpAddr::from([203, 0, 113, 7]));
    }

    #[test]
    fn cloudflares_header_is_ignored_unless_behind_a_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.7"));
        let mut extensions = Extensions::new();
        extensions.insert(ConnectInfo(SocketAddr::from(([198, 51, 100, 9], 4000))));
        assert_eq!(
            client_ip(&headers, &extensions, false),
            IpAddr::from([198, 51, 100, 9])
        );
    }

    #[test]
    fn the_rightmost_public_address_is_the_players() {
        // Render's internal hops come after the address it saw
        assert_eq!(
            from("203.0.113.7, 10.1.2.3", true),
            IpAddr::from([203, 0, 113, 7])
        );
    }

    #[test]
    fn an_address_the_visitor_wrote_is_ignored() {
        // A cheater puts a fake address first. The proxy's entry comes after it
        assert_eq!(
            from("1.2.3.4, 203.0.113.7, 10.1.2.3", true),
            IpAddr::from([203, 0, 113, 7])
        );
    }

    #[test]
    fn the_header_is_ignored_unless_behind_a_proxy() {
        assert_eq!(from("203.0.113.7", false), IpAddr::from([10, 0, 0, 5]));
    }

    #[test]
    fn entries_with_a_port_are_read() {
        assert_eq!(
            from("203.0.113.7:5678", true),
            IpAddr::from([203, 0, 113, 7])
        );
        let v6: IpAddr = "2001:db8::1".parse().unwrap();
        assert_eq!(from("[2001:db8::1]:443", true), v6);
    }

    #[test]
    fn a_private_address_written_as_ipv6_is_still_private() {
        assert_eq!(
            from("203.0.113.7, ::ffff:10.1.2.3", true),
            IpAddr::from([203, 0, 113, 7])
        );
    }

    #[test]
    fn reserved_ranges_are_proxy_hops_not_players() {
        for hop in [
            "198.18.0.5",
            "192.0.0.9",
            "240.1.2.3",
            "0.1.2.3",
            "100.64.0.1",
        ] {
            let forwarded = format!("203.0.113.7, {hop}");
            assert_eq!(
                from(&forwarded, true),
                IpAddr::from([203, 0, 113, 7]),
                "{hop}"
            );
        }
    }

    #[test]
    fn a_proxy_on_this_computer_doesnt_skip_the_limits() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.7"));
        let mut extensions = Extensions::new();
        extensions.insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4000))));
        let ip = client_ip(&headers, &extensions, false);
        assert!(!ip.is_loopback());
        // Without the header it really is this computer
        let ip = client_ip(&HeaderMap::new(), &extensions, false);
        assert!(ip.is_loopback());
    }

    #[test]
    fn without_a_public_entry_the_connection_counts() {
        assert_eq!(
            from("garbage, 192.168.1.4", true),
            IpAddr::from([10, 0, 0, 5])
        );
    }
}
