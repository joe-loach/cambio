mod db;
mod error;
mod id;
mod log;
mod middleware;
mod models;
mod routes;
mod token;

use id::Id;
use middleware::auth;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use axum::{
    http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        Method,
    },
    routing::{get, post},
    Router,
};

pub struct AppState<'a> {
    db: db::Db<'a>,
}

fn router(state: Arc<AppState<'static>>) -> Router {
    // let origins = HeaderValue::from_str("http://localhost:3000").unwrap();
    let cors = CorsLayer::new()
        .allow_origin([])
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    // routes that require auth
    let auth_routes = Router::new()
        .route("/create", post(routes::create::create_game))
        .route("/join/{game_id}", get(routes::join::join_game))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth,
        ));

    Router::new()
        .route("/list", get(routes::list::game_list))
        .route("/register", post(routes::register::register_user_handler))
        .route("/login", get(routes::login::login_handler))
        .merge(auth_routes)
        .with_state(state)
        .layer(log::layer())
        .layer(cors)
}

#[tokio::main]
async fn main() {
    log::register();

    dotenvy::dotenv().expect("failed to load config");

    let db = db::establish_connection().expect("failed to connect to database");

    let state = Arc::new(AppState { db });

    let app = router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap()
}
