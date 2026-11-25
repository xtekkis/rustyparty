// The router's rules for which websites may call it, and how often
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use tower::ServiceExt;

use super::router;
use crate::AppState;
use crate::origins::AllowedOrigins;

const FRONTEND: &str = "https://party.example";

fn app(origins: AllowedOrigins) -> Router {
    router(
        AppState {
            origins,
            ..AppState::default()
        },
        None,
    )
}

fn only_frontend() -> Router {
    app(AllowedOrigins::parse(FRONTEND).unwrap())
}

async fn health(app: Router, origin: &str) -> Response {
    let request = Request::get("/health")
        .header(header::ORIGIN, origin)
        .body(Body::empty())
        .unwrap();
    app.oneshot(request).await.unwrap()
}

// A browser's WebSocket request. It can't really upgrade here, so it only shows whether
// the website check let it through
async fn websocket(app: Router, origin: Option<&str>) -> StatusCode {
    let mut request = Request::get("/ws")
        .header(header::CONNECTION, "upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, "dGhlIHNhbXBsZSBub25jZQ==");
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    let response = app.oneshot(request.body(Body::empty()).unwrap()).await;
    response.unwrap().status()
}

fn allowed_origin(response: &Response) -> Option<&str> {
    response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
        .map(|value| value.to_str().unwrap())
}

#[tokio::test(start_paused = true)]
async fn the_frontend_may_read_responses() {
    let response = health(only_frontend(), FRONTEND).await;
    assert_eq!(allowed_origin(&response), Some(FRONTEND));
}

#[tokio::test(start_paused = true)]
async fn other_websites_may_not() {
    let response = health(only_frontend(), "https://evil.example").await;
    assert_eq!(allowed_origin(&response), None);
}

#[tokio::test(start_paused = true)]
async fn any_website_may_when_the_setting_is_missing() {
    let response = health(app(AllowedOrigins::Any), "https://evil.example").await;
    assert_eq!(allowed_origin(&response), Some("*"));
}

#[tokio::test(start_paused = true)]
async fn a_websocket_from_another_website_is_refused() {
    let status = websocket(only_frontend(), Some("https://evil.example")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test(start_paused = true)]
async fn a_websocket_from_the_frontend_gets_through() {
    assert_ne!(
        websocket(only_frontend(), Some(FRONTEND)).await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test(start_paused = true)]
async fn a_websocket_without_a_website_gets_through() {
    // Apps and scripts send no Origin
    assert_ne!(
        websocket(only_frontend(), None).await,
        StatusCode::FORBIDDEN
    );
}

// Behind Render's proxy, so a test picks its address with X-Forwarded-For
fn behind_proxy() -> Router {
    router(
        AppState {
            behind_proxy: true,
            ..AppState::default()
        },
        None,
    )
}

async fn send(app: &Router, method: &str, path: &str, address: &str) -> StatusCode {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-forwarded-for", address)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(request).await.unwrap().status()
}

#[tokio::test(start_paused = true)]
async fn creating_rooms_is_limited_per_address() {
    let app = behind_proxy();
    for _ in 0..3 {
        assert_eq!(
            send(&app, "POST", "/rooms", "203.0.113.7").await,
            StatusCode::CREATED
        );
    }
    assert_eq!(
        send(&app, "POST", "/rooms", "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
    // Someone else isn't held up
    assert_eq!(
        send(&app, "POST", "/rooms", "198.51.100.9").await,
        StatusCode::CREATED
    );
}

#[tokio::test(start_paused = true)]
async fn a_fake_address_doesnt_get_round_the_limit() {
    let app = behind_proxy();
    for n in 0..3 {
        // The cheater writes a new address each time, but the proxy's entry is last
        let forwarded = format!("1.2.3.{n}, 203.0.113.7");
        assert_eq!(
            send(&app, "POST", "/rooms", &forwarded).await,
            StatusCode::CREATED
        );
    }
    let status = send(&app, "POST", "/rooms", "1.2.3.99, 203.0.113.7").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test(start_paused = true)]
async fn looking_up_rooms_is_limited_per_address() {
    let app = behind_proxy();
    for _ in 0..30 {
        assert_eq!(
            send(&app, "GET", "/rooms/ABCD", "203.0.113.7").await,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        send(&app, "GET", "/rooms/ABCD", "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test(start_paused = true)]
async fn opening_websockets_is_limited_per_address() {
    let app = behind_proxy();
    for _ in 0..30 {
        assert_ne!(
            send(&app, "GET", "/ws", "203.0.113.7").await,
            StatusCode::TOO_MANY_REQUESTS
        );
    }
    assert_eq!(
        send(&app, "GET", "/ws", "203.0.113.7").await,
        StatusCode::TOO_MANY_REQUESTS
    );
}

#[tokio::test(start_paused = true)]
async fn looking_up_a_real_room_never_counts() {
    let app = behind_proxy();
    let request = Request::post("/rooms")
        .header("x-forwarded-for", "203.0.113.7")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let room: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let path = format!("/rooms/{}", room["code"].as_str().unwrap());

    // Ten phones checking the invite link, several times over
    for _ in 0..40 {
        assert_eq!(
            send(&app, "GET", &path, "203.0.113.7").await,
            StatusCode::OK
        );
    }
}

#[tokio::test(start_paused = true)]
async fn an_address_out_of_tries_cant_tell_real_rooms_apart() {
    let app = behind_proxy();
    let request = Request::post("/rooms")
        .header("x-forwarded-for", "203.0.113.7")
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let room: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let path = format!("/rooms/{}", room["code"].as_str().unwrap());

    // Someone on the same address guesses until they're out of tries
    for _ in 0..30 {
        assert_eq!(
            send(&app, "GET", "/rooms/ZZZZ", "203.0.113.7").await,
            StatusCode::NOT_FOUND
        );
    }
    let request = Request::get("/rooms/ZZZZ")
        .header("x-forwarded-for", "203.0.113.7")
        .body(Body::empty())
        .unwrap();
    let refused = app.clone().oneshot(request).await.unwrap();
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(refused.headers()[header::RETRY_AFTER], "2");

    // A real room is refused too, the same way, so the answer gives nothing away
    let request = Request::get(path.as_str())
        .header("x-forwarded-for", "203.0.113.7")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}

async fn body_of(app: &Router, method: &str, path: &str) -> (StatusCode, String) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    (status, String::from_utf8_lossy(&body).into_owned())
}

// A folder for one test, deleted afterwards even if the test fails
struct TempFolder(std::path::PathBuf);

impl TempFolder {
    fn new() -> TempFolder {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("rustyparty-test-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        TempFolder(path)
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

#[tokio::test(start_paused = true)]
async fn the_frontend_is_served_with_the_game_server() {
    let temp = TempFolder::new();
    let site = temp.0.join("site");
    std::fs::create_dir_all(site.join("assets")).unwrap();
    std::fs::write(site.join("index.html"), "<p>the app</p>").unwrap();
    std::fs::write(site.join("assets/app.js"), "console.log('app')").unwrap();
    // Next to the frontend folder, never to be served
    std::fs::write(temp.0.join("secret.txt"), "secret").unwrap();
    let app = router(AppState::default(), Some(&site));
    let page = (StatusCode::OK, "<p>the app</p>".to_string());

    assert_eq!(body_of(&app, "GET", "/").await, page);
    // Invite links open the app, which reads the code from the address
    assert_eq!(body_of(&app, "GET", "/join/KXTP").await, page);
    assert_eq!(
        body_of(&app, "GET", "/assets/app.js").await,
        (StatusCode::OK, "console.log('app')".into())
    );
    // A missing file is a plain 404, never the app's HTML
    assert_eq!(
        body_of(&app, "GET", "/assets/old-name.js").await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        body_of(&app, "GET", "/favicon.ico").await.0,
        StatusCode::NOT_FOUND
    );
    // The game server's own routes come first, and wrong game server paths are 404 too
    assert_eq!(
        body_of(&app, "GET", "/health").await,
        (StatusCode::OK, "ok".into())
    );
    let (status, body) = body_of(&app, "GET", "/rooms/ABCD").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body.contains("room_not_found"));
    assert_eq!(
        body_of(&app, "GET", "/rooms/ABCD/extra").await.0,
        StatusCode::NOT_FOUND
    );
    // Nothing outside the folder
    assert_eq!(
        body_of(&app, "GET", "/../secret.txt").await,
        (StatusCode::NOT_FOUND, String::new())
    );
}
#[tokio::test(start_paused = true)]
async fn without_a_frontend_only_the_game_server_answers() {
    let app = router(AppState::default(), None);
    assert_eq!(body_of(&app, "GET", "/").await.0, StatusCode::NOT_FOUND);
    assert_eq!(body_of(&app, "GET", "/health").await.0, StatusCode::OK);
}

async fn headers_of(app: &Router, path: &str) -> axum::http::HeaderMap {
    let request = Request::get(path).body(Body::empty()).unwrap();
    app.clone()
        .oneshot(request)
        .await
        .unwrap()
        .headers()
        .clone()
}

#[tokio::test(start_paused = true)]
async fn every_response_has_the_security_and_cache_headers() {
    let temp = TempFolder::new();
    std::fs::create_dir_all(temp.0.join("assets")).unwrap();
    std::fs::write(temp.0.join("index.html"), "<p>the app</p>").unwrap();
    std::fs::write(temp.0.join("assets/app-1234.js"), "console.log('app')").unwrap();
    let app = router(AppState::default(), Some(&temp.0));

    let page = headers_of(&app, "/join/KXTP").await;
    let policy = page[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
    assert!(policy.contains("default-src 'self'"));
    assert!(policy.contains("frame-ancestors 'none'"));
    assert_eq!(page[header::X_FRAME_OPTIONS], "DENY");
    assert_eq!(page[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(page[header::REFERRER_POLICY], "no-referrer");
    // A page is checked every time, so a deploy is picked up straight away
    assert_eq!(page[header::CACHE_CONTROL], "no-cache");

    // Built files never change, so they're kept
    let built = headers_of(&app, "/assets/app-1234.js").await;
    assert_eq!(
        built[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    // But a missing one isn't
    let missing = headers_of(&app, "/assets/gone-5678.js").await;
    assert_eq!(missing[header::CACHE_CONTROL], "no-cache");

    // The game server's answers have them too
    let api = headers_of(&app, "/health").await;
    assert_eq!(api[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(api[header::CACHE_CONTROL], "no-cache");
}
