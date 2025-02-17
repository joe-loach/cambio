use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use thiserror::Error;

use crate::{db, id::Id, models::game::Game, AppState};

#[derive(Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct JoinGameResponse {
    pub(crate) server_addr: SocketAddr,
}

#[derive(Debug, Error)]
pub enum JoinError {
    #[error(transparent)]
    Db(#[from] db::DbError),
    #[error("No game found")]
    NotFound,
}

pub async fn join_game(
    Path(game_id): Path<Id>,
    State(state): State<Arc<AppState<'_>>>,
) -> Result<Json<JoinGameResponse>, JoinError> {
    let r = state.db.read()?;

    if let Some(game) = r.get().primary::<Game>(game_id)? {
        Ok(Json(JoinGameResponse {
            server_addr: game.info.server_addr,
        }))
    } else {
        Err(JoinError::NotFound)
    }
}

impl IntoResponse for JoinError {
    fn into_response(self) -> axum::response::Response {
        match self {
            JoinError::Db(db_error) => db_error.into_response(),
            JoinError::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}
