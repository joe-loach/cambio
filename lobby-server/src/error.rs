use axum::http::StatusCode;

pub const INTERNAL_ERROR: (StatusCode, &str) = (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong");