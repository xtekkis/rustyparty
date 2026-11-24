use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

// Which websites may call the server: any, or only the ones in ALLOWED_ORIGINS
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AllowedOrigins {
    // For development, where the frontend's address changes (localhost, a phone on Wi-Fi)
    #[default]
    Any,
    Only(Arc<[HeaderValue]>),
}

impl AllowedOrigins {
    // From ALLOWED_ORIGINS, like "https://rustyparty.example,https://www.rustyparty.example".
    // A wrong value stops the server here, rather than letting every website in
    pub fn from_env() -> Self {
        match std::env::var("ALLOWED_ORIGINS") {
            Ok(value) => {
                Self::parse(&value).unwrap_or_else(|error| panic!("ALLOWED_ORIGINS {error}"))
            }
            Err(_) => Self::Any,
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        let mut origins = Vec::new();
        for origin in value
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
        {
            // Browsers send the scheme and host in lowercase
            let origin = origin.to_ascii_lowercase();
            if !is_origin(&origin) {
                return Err(format!(
                    "has {origin:?}, each must look like https://example.com, with no path"
                ));
            }
            let header = HeaderValue::from_str(&origin)
                .map_err(|_| format!("has {origin:?}, which isn't a valid address"))?;
            origins.push(header);
        }
        if origins.is_empty() {
            return Err("is set but has no addresses".to_string());
        }
        Ok(Self::Only(origins.into()))
    }

    pub fn allows(&self, origin: &HeaderValue) -> bool {
        match self {
            Self::Any => true,
            Self::Only(origins) => origins.contains(origin),
        }
    }

    // What browsers are told about the HTTP routes. The WebSocket is checked separately
    pub fn cors(&self) -> CorsLayer {
        let layer = CorsLayer::new().allow_methods([Method::GET, Method::POST]);
        match self {
            Self::Any => layer.allow_origin(Any),
            Self::Only(origins) => layer.allow_origin(AllowOrigin::list(origins.iter().cloned())),
        }
    }
}

// A scheme and a host, with an optional port: https://example.com or http://localhost:5173
fn is_origin(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    !rest.is_empty() && !rest.contains(['/', '?', '#', '@', ' '])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(origins: &[&'static str]) -> AllowedOrigins {
        AllowedOrigins::Only(
            origins
                .iter()
                .map(|o| HeaderValue::from_static(o))
                .collect(),
        )
    }

    #[test]
    fn a_list_is_read_with_spaces_and_capitals() {
        assert_eq!(
            AllowedOrigins::parse(" https://Party.example , http://localhost:5173,").unwrap(),
            only(&["https://party.example", "http://localhost:5173"])
        );
    }

    #[test]
    fn addresses_with_a_path_or_no_scheme_are_refused() {
        assert!(AllowedOrigins::parse("https://party.example/").is_err());
        assert!(AllowedOrigins::parse("https://party.example/join").is_err());
        assert!(AllowedOrigins::parse("party.example").is_err());
        assert!(AllowedOrigins::parse("https://").is_err());
    }

    #[test]
    fn an_empty_setting_is_refused() {
        assert!(AllowedOrigins::parse("").is_err());
        assert!(AllowedOrigins::parse(" , ").is_err());
    }

    #[test]
    fn only_listed_websites_are_allowed() {
        let origins = only(&["https://party.example"]);
        assert!(origins.allows(&HeaderValue::from_static("https://party.example")));
        assert!(!origins.allows(&HeaderValue::from_static("https://evil.example")));
        assert!(!origins.allows(&HeaderValue::from_static("http://party.example")));
        assert!(AllowedOrigins::Any.allows(&HeaderValue::from_static("https://evil.example")));
    }
}
