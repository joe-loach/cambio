use std::{
    env,
    sync::{Arc, LazyLock},
};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Serialize;
use thiserror::Error;

use crate::{
    error::INTERNAL_ERROR,
    token::{RefreshClaim, REFRESH_TOKEN_COOKIE},
    AppState,
};

static REFRESH_DECODING_KEY: LazyLock<DecodingKey> = LazyLock::new(|| {
    DecodingKey::from_secret(
        env::var("REFRESH_TOKEN_SECRET")
            .expect("Secret in config")
            .as_bytes(),
    )
});

#[derive(Serialize)]
pub struct RefreshResponse {
    access_token: String,
}

#[derive(Debug, Error)]
pub enum RefreshError {
    #[error("No refresh token")]
    NoToken,
    #[error(transparent)]
    JwtEncode(#[from] jsonwebtoken::errors::Error),
}

pub async fn refresh_token(
    State(_state): State<Arc<AppState<'_>>>,
    jar: CookieJar,
) -> Result<(CookieJar, Json<RefreshResponse>), RefreshError> {
    let refresh_token = jar.get(REFRESH_TOKEN_COOKIE).ok_or(RefreshError::NoToken)?;

    let token_data = decode::<RefreshClaim>(
        refresh_token.value(),
        &REFRESH_DECODING_KEY,
        &Validation::default(),
    )?;

    let issued_at = jsonwebtoken::get_current_timestamp();

    let access_token = crate::token::encode_access_token(issued_at, token_data.claims.sub.clone())?;
    // let refresh_token = crate::token::refresh_token(issued_at, token_data.claims.sub)?;

    Ok((jar, Json(RefreshResponse { access_token })))
}

impl IntoResponse for RefreshError {
    fn into_response(self) -> Response {
        match self {
            RefreshError::NoToken => {
                (StatusCode::UNAUTHORIZED, "No refresh token provided").into_response()
            }
            RefreshError::JwtEncode(_) => INTERNAL_ERROR.into_response(),
        }
    }
}
