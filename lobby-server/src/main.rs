mod db;
mod models;
mod id;
mod routes;

use id::Id;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Extension, Router,
};

pub struct AppState<'a> {
    db: db::Db<'a>,
}

#[tokio::main]
async fn main() {
    let db = db::establish_connection().expect("failed to connect to database");

    let state = Arc::new(AppState { db });

    let app = Router::new()
        .route("/create", post(routes::create::create_game))
        .route("/join/{game_id}", get(routes::join::join_game))
        .route("/list", get(routes::list::game_list))
        .layer(Extension(state));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap()
}
