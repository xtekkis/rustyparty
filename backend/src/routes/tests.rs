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
    router(AppState {
        origins,
        ..AppState::default()
    })
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
    router(AppState {
        behind_proxy: true,
        ..AppState::default()
    })
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
    for _ in 0..5 {
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
    for n in 0..5 {
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
async fn a_real_room_is_never_refused_even_after_guessing() {
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

    // The real room still answers
    assert_eq!(
        send(&app, "GET", &path, "203.0.113.7").await,
        StatusCode::OK
    );
}
