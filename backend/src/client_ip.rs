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
    if behind_proxy && let Some(ip) = forwarded_ip(headers) {
        return ip;
    }
    if !behind_proxy && headers.contains_key("x-forwarded-for") {
        warn_about_proxy();
    }
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
}

// The proxy adds the address it saw to the end of X-Forwarded-For, after anything the visitor
// wrote themselves, then its own internal hops. So the rightmost public address is the real one
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
            "Requests come through a proxy but BEHIND_PROXY is not set, so players may share one limit. Set BEHIND_PROXY=true"
        );
    }
}

fn is_public(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let shared = v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64;
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || shared)
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local())
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
    fn without_a_public_entry_the_connection_counts() {
        assert_eq!(
            from("garbage, 192.168.1.4", true),
            IpAddr::from([10, 0, 0, 5])
        );
    }
}
