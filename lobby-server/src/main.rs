mod game;
mod id;

use game::Game;
use id::Id;
use parking_lot::RwLock;
use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};

type GameList = Arc<RwLock<Vec<Game>>>;

#[tokio::main]
async fn main() {
    let games = GameList::default();

    let app = Router::new()
        .route("/create", post(create_game))
        .route("/join/{game_id}", get(join_game))
        .route("/list", get(game_list))
        .layer(Extension(games));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap()
}

async fn create_game(
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

async fn join_game(
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

async fn game_list(Extension(games): Extension<GameList>) -> Json<GameListResponse> {
    let game_info = games
        .read()
        .iter()
        .filter(|g| g.is_public())
        .map(|g| g.info.clone())
        .collect();

    Json(GameListResponse { game_info })
}

#[derive(Serialize)]
struct GameListResponse {
    game_info: Vec<game::GameInfo>,
}

#[derive(Deserialize)]
struct CreateGameRequest {
    name: String,
    visibility: game::Visibility,
    server_addr: SocketAddr,
}

#[derive(Serialize)]
struct CreateGameResponse {
    id: Id,
}

#[derive(Serialize)]
struct JoinGameResponse {
    server_addr: SocketAddr,
}
