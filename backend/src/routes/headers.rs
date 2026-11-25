use axum::extract::Request;
use axum::http::HeaderValue;
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, HeaderName, REFERRER_POLICY, STRICT_TRANSPORT_SECURITY,
    X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::middleware::Next;
use axum::response::Response;

// Everything the app loads comes from this site: its one script, stylesheet, fonts and icons,
// the game server and its WebSocket. Nothing else may run, load, frame it or be posted to
const CONTENT_SECURITY_POLICY_VALUE: &str = "default-src 'self'; script-src 'self'; \
    style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self'; \
    manifest-src 'self'; media-src 'self'; object-src 'none'; base-uri 'none'; \
    form-action 'none'; frame-ancestors 'none'";

// Sent with every response
const SECURITY: [(HeaderName, &str); 7] = [
    (CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY_VALUE),
    // No other site may show the game inside a frame, for browsers older than frame-ancestors
    (X_FRAME_OPTIONS, "DENY"),
    // A file is only what its type says, never guessed to be a script
    (X_CONTENT_TYPE_OPTIONS, "nosniff"),
    // Page addresses hold room codes, so they aren't passed on to other sites
    (REFERRER_POLICY, "no-referrer"),
    // The camera, for scanning a room's QR code, and nothing else
    (
        HeaderName::from_static("permissions-policy"),
        "camera=(self), microphone=(), geolocation=(), payment=(), usb=()",
    ),
    // Once a phone has seen the site over https, it never goes back to plain http.
    // Browsers ignore this over plain http, so development is unaffected
    (STRICT_TRANSPORT_SECURITY, "max-age=31536000"),
    // Other windows can't reach into the page
    (
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    ),
];

// Built files have their content in their names, so they never change and can be kept a year.
// Everything else is checked every time, so after a deploy no phone keeps an old index.html
// asking for files that are gone
const CACHE_FOREVER: &str = "public, max-age=31536000, immutable";
const CHECK_EVERY_TIME: &str = "no-cache";

pub async fn add_headers(request: Request, next: Next) -> Response {
    let built_file = request.uri().path().starts_with("/assets/");
    let mut response = next.run(request).await;
    let found = response.status().is_success();
    let headers = response.headers_mut();
    for (name, value) in SECURITY {
        headers.insert(name, HeaderValue::from_static(value));
    }
    let cache = if built_file && found {
        CACHE_FOREVER
    } else {
        CHECK_EVERY_TIME
    };
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}
