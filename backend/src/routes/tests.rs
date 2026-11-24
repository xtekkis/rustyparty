// The router's rules for which websites may call it
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

#[tokio::test]
async fn the_frontend_may_read_responses() {
    let response = health(only_frontend(), FRONTEND).await;
    assert_eq!(allowed_origin(&response), Some(FRONTEND));
}

#[tokio::test]
async fn other_websites_may_not() {
    let response = health(only_frontend(), "https://evil.example").await;
    assert_eq!(allowed_origin(&response), None);
}

#[tokio::test]
async fn any_website_may_when_the_setting_is_missing() {
    let response = health(app(AllowedOrigins::Any), "https://evil.example").await;
    assert_eq!(allowed_origin(&response), Some("*"));
}

#[tokio::test]
async fn a_websocket_from_another_website_is_refused() {
    let status = websocket(only_frontend(), Some("https://evil.example")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_websocket_from_the_frontend_gets_through() {
    assert_ne!(
        websocket(only_frontend(), Some(FRONTEND)).await,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn a_websocket_without_a_website_gets_through() {
    // Apps and scripts send no Origin
    assert_ne!(
        websocket(only_frontend(), None).await,
        StatusCode::FORBIDDEN
    );
}
