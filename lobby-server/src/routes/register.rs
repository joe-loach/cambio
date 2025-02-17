use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::{db, error::INTERNAL_ERROR, models::user::User, AppState};

#[derive(Deserialize)]
pub struct RegisterRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct RegisterResponse {}

#[derive(Debug, Error)]
pub enum RegisterError {
    #[error(transparent)]
    PasswordHash(#[from] argon2::password_hash::Error),
    #[error(transparent)]
    Db(#[from] db::DbError),
    #[error("User already exists")]
    AlreadyExists,
}

pub async fn register_user_handler(
    State(state): State<Arc<AppState<'_>>>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, RegisterError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(body.password.as_bytes(), &salt)?
        .to_string();

    let new_user = User {
        name: body.username,
        password: password_hash,
    };

    let rw = state.db.read_write()?;
    if rw.insert(new_user).is_err() {
        return Err(RegisterError::AlreadyExists);
    }
    rw.commit()?;

    Ok(Json(RegisterResponse {}))
}

impl IntoResponse for RegisterError {
    fn into_response(self) -> axum::response::Response {
        match self {
            RegisterError::PasswordHash(error) => {
                tracing::error!(error = %error);

                INTERNAL_ERROR.into_response()
            }
            RegisterError::Db(db_error) => db_error.into_response(),
            RegisterError::AlreadyExists => {
                (StatusCode::BAD_REQUEST, "Failed to create user".to_owned()).into_response()
            }
        }
    }
}
