use axum::{Extension, Json};
use serde::Serialize;

use crate::{game, GameList};

#[derive(Serialize)]
pub struct GameListResponse {
    game_info: Vec<game::GameInfo>,
}

pub async fn game_list(Extension(games): Extension<GameList>) -> Json<GameListResponse> {
    let game_info = games
        .read()
        .iter()
        .filter(|g| g.is_public())
        .map(|g| g.info.clone())
        .collect();

    Json(GameListResponse { game_info })
}
