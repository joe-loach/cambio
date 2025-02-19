use std::{
    env,
    sync::{Arc, LazyLock},
};

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::IntoResponse,
};

use jsonwebtoken::{decode, DecodingKey, Validation};
use thiserror::Error;

use crate::{db, error::INTERNAL_ERROR, models::user::User, token::AccessClaim, AppState};

static ACCESS_DECODING_KEY: LazyLock<DecodingKey> = LazyLock::new(|| {
    DecodingKey::from_secret(
        env::var("ACCESS_TOKEN_SECRET")
            .expect("Secret in config")
            .as_bytes(),
    )
});

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("User is unauthorized")]
    Unauthorized,
    #[error(transparent)]
    JwtDecode(#[from] jsonwebtoken::errors::Error),
    #[error(transparent)]
    Db(#[from] db::DbError),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AuthError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "You must provide a valid token").into_response()
            }
            AuthError::JwtDecode(_) => INTERNAL_ERROR.into_response(),
            AuthError::Db(db_error) => db_error.into_response(),
        }
    }
}

pub async fn auth(
    State(state): State<Arc<AppState<'_>>>,
    mut req: Request,
    next: Next,
) -> Result<impl IntoResponse, AuthError> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .ok_or(AuthError::Unauthorized)?;

    let token = decode::<AccessClaim>(token, &ACCESS_DECODING_KEY, &Validation::default())?;

    let r = state.db.read()?;
    let Some(user) = r.get().primary::<User>(token.claims.sub)? else {
        return Err(AuthError::Unauthorized);
    };
    req.extensions_mut().insert(user);

    Ok(next.run(req).await)
}
