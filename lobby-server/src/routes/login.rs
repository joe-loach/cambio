use argon2::{password_hash, Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jiff::{Timestamp, ToSpan};
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

    let iat = Timestamp::now();
    let expiry_time = 7.days();
    let exp = iat + expiry_time;

    let claims = TokenClaim {
        sub: user.name,
        // convert timestamps to seconds since unix epoch as per
        // https://www.rfc-editor.org/rfc/rfc7519#section-2
        exp: exp.as_second().unsigned_abs(),
        iat: iat.as_second().unsigned_abs(),
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
