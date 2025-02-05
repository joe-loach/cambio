use std::net::SocketAddr;

use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use crate::{game, id::Id, Game, GameList};

#[derive(Deserialize)]
pub struct CreateGameRequest {
    name: String,
    visibility: game::Visibility,
    server_addr: SocketAddr,
}

#[derive(Serialize)]
pub struct CreateGameResponse {
    id: Id,
}

pub async fn create_game(
    Extension(games): Extension<GameList>,
    Json(CreateGameRequest {
        name,
        visibility,
        server_addr,
    }): Json<CreateGameRequest>,
) -> Json<CreateGameResponse> {
    let game_id = Id::new();

    let new_game = Game::new(
        game_id.clone(),
        visibility,
        game::GameInfo { name, server_addr },
    );

    games.write().push(new_game);

    Json(CreateGameResponse { id: game_id })
}
