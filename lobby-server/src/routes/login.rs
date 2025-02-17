use argon2::{password_hash, Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::{cookie::Cookie, CookieJar};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::{db, error::INTERNAL_ERROR, models::user::User, token::REFRESH_TOKEN_COOKIE, AppState};

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Deserialize))]
pub struct LoginResponse {
    pub(crate) access_token: String,
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

pub async fn login_handler(
    State(state): State<Arc<AppState<'_>>>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>), LoginError> {
    let r = state.db.read()?;

    let Some(user) = r.get().primary::<User>(body.username)? else {
        return Err(LoginError::NoUserWithId);
    };

    let hash = PasswordHash::new(&user.password)?;
    Argon2::default().verify_password(body.password.as_bytes(), &hash)?;

    let issued_at = jsonwebtoken::get_current_timestamp();

    let access_token = crate::token::encode_access_token(issued_at, user.name.clone())?;
    let refresh_token = crate::token::encode_refresh_token(issued_at, user.name)?;

    let jar = jar.add(Cookie::new(REFRESH_TOKEN_COOKIE, refresh_token));

    Ok((jar, Json(LoginResponse { access_token })))
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
