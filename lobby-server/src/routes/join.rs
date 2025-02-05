use std::net::SocketAddr;

use axum::{extract::Path, http::StatusCode, Extension, Json};
use serde::Serialize;

use crate::{id::Id, GameList};

#[derive(Serialize)]
pub struct JoinGameResponse {
    server_addr: SocketAddr,
}

pub async fn join_game(
    Path(game_id): Path<Id>,
    Extension(games): Extension<GameList>,
) -> Result<Json<JoinGameResponse>, StatusCode> {
    let games = games.read();

    if let Some(game) = games.iter().find(|g| g.id == game_id) {
        Ok(Json(JoinGameResponse {
            server_addr: game.info.server_addr,
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
