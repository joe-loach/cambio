use std::{net::SocketAddr, sync::Arc};

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::{
    db::DbError,
    id::Id,
    models::game::{self, Game},
    AppState,
};

#[derive(Deserialize)]
pub struct CreateGameRequest {
    name: String,
    visibility: game::Visibility,
    server_addr: SocketAddr,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(Deserialize))]
pub struct CreateGameResponse {
    pub(crate) id: Id,
}

pub async fn create_game(
    State(state): State<Arc<AppState<'_>>>,
    Json(CreateGameRequest {
        name,
        visibility,
        server_addr,
    }): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, DbError> {
    let game_id = Id::new();

    let new_game = Game::new(
        game_id.clone(),
        visibility,
        game::GameInfo { name, server_addr },
    );

    let rw = state.db.read_write()?;
    rw.insert(new_game)?;
    rw.commit()?;

    Ok(Json(CreateGameResponse { id: game_id }))
}
