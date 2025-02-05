mod game;
mod id;
mod routes;

use game::Game;
use id::Id;
use parking_lot::RwLock;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Extension, Router,
};

type GameList = Arc<RwLock<Vec<Game>>>;

#[tokio::main]
async fn main() {
    let games = GameList::default();

    let app = Router::new()
        .route("/create", post(routes::create::create_game))
        .route("/join/{game_id}", get(routes::join::join_game))
        .route("/list", get(routes::list::game_list))
        .layer(Extension(games));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap()
}
