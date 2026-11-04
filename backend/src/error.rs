use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

// Errors the server can send back to the client
#[derive(Debug)]
pub enum AppError {
    RoomNotFound,
    NoFreeRoomCode,
}

// JSON body for every error
#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
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
        };

        (status, Json(ErrorBody { code, message })).into_response()
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
    fn no_free_room_code_is_503() {
        let response = AppError::NoFreeRoomCode.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
