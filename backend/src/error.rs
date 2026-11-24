use axum::{
    Json,
    http::{StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};
use serde::Serialize;

// Errors the server can send back to the client
#[derive(Debug)]
pub enum AppError {
    RoomNotFound,
    NoFreeRoomCode,
    // With how many seconds until a try is back
    TooManyRequests { retry_after: u64 },
}

// JSON body for every error
#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let retry_after = match self {
            AppError::TooManyRequests { retry_after } => Some(retry_after),
            _ => None,
        };
        let (status, code, message) = match self {
            AppError::RoomNotFound => (
                StatusCode::NOT_FOUND,
                "room_not_found",
                "No room with that code",
            ),
            AppError::NoFreeRoomCode => (
                StatusCode::SERVICE_UNAVAILABLE,
                "rooms_full",
                "The server is full, try again in a few minutes",
            ),
            AppError::TooManyRequests { .. } => (
                StatusCode::TOO_MANY_REQUESTS,
                "too_many_requests",
                "Too many tries, wait a moment and try again",
            ),
        };

        let mut response = (status, Json(ErrorBody { code, message })).into_response();
        if let Some(seconds) = retry_after {
            response.headers_mut().insert(RETRY_AFTER, seconds.into());
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_not_found_is_404() {
        let response = AppError::RoomNotFound.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn too_many_requests_says_when_to_try_again() {
        let response = AppError::TooManyRequests { retry_after: 20 }.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers()[RETRY_AFTER], "20");
    }

    #[test]
    fn no_free_room_code_is_503() {
        let response = AppError::NoFreeRoomCode.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
