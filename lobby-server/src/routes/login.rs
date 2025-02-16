use argon2::{password_hash, Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::{
    env,
    sync::{Arc, LazyLock},
};
use thiserror::Error;

use crate::{db, error::INTERNAL_ERROR, models::user::User, token::TokenClaim, AppState};

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    token: String,
}

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("User doesn't exist")]
    NoUserWithId,
    #[error(transparent)]
    Db(#[from] db::DbError),
    #[error(transparent)]
    HashError(#[from] password_hash::Error),
    #[error(transparent)]
    JwtEncode(#[from] jsonwebtoken::errors::Error),
}

static ENCODING_KEY: LazyLock<EncodingKey> = LazyLock::new(|| {
    EncodingKey::from_secret(env::var("JWT_SECRET").expect("Secret in config").as_bytes())
});

pub async fn login_handler(
    State(state): State<Arc<AppState<'_>>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, LoginError> {
    let r = state.db.read()?;

    let Some(user) = r.get().primary::<User>(body.username)? else {
        return Err(LoginError::NoUserWithId);
    };

    let hash = PasswordHash::new(&user.password)?;
    Argon2::default().verify_password(body.password.as_bytes(), &hash)?;

    let iat = jsonwebtoken::get_current_timestamp();
    let expiry_time = time::Duration::days(7);
    let exp = iat.saturating_add(expiry_time.whole_seconds().unsigned_abs());

    let claims = TokenClaim {
        sub: user.name,
        exp,
        iat,
    };

    let token = encode(&Header::default(), &claims, &ENCODING_KEY)?;

    Ok(Json(LoginResponse { token }))
}

impl IntoResponse for LoginError {
    fn into_response(self) -> Response {
        match self {
            LoginError::NoUserWithId => {
                (StatusCode::BAD_REQUEST, "User does not exist").into_response()
            }
            LoginError::Db(db_error) => db_error.into_response(),
            _ => INTERNAL_ERROR.into_response(),
        }
    }
}
