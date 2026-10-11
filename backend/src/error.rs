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
}
