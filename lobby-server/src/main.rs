#[cfg(test)]
mod integration;

mod middleware;
mod log;
mod limiter;

use lobby_server::*;
use middleware::auth;
use std::{net::SocketAddr, sync::Arc};
use tower_governor::GovernorLayer;
use tower_http::cors::CorsLayer;

use axum::{
    http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        Method,
    },
    routing::{any, get, post},
    Router,
};

fn router(state: Arc<AppState<'static>>) -> Router {
    // let origins = HeaderValue::from_str("http://localhost:3000").unwrap();
    let cors = CorsLayer::new()
        .allow_origin([])
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_credentials(true)
        .allow_headers([AUTHORIZATION, ACCEPT, CONTENT_TYPE]);

    // Create rate limiter
    let secure_governor = Arc::new(limiter::secure());
    tokio::spawn({
        let limiter = secure_governor.limiter().clone();
        limiter::cleanup_limiter_task(move || limiter.retain_recent())
    });

    // Routes that provide authorization
    let authorization_providers = Router::new()
        .route("/register", post(routes::register::register_user_handler))
        .route("/login", get(routes::login::login_handler))
        .route("/refresh", get(routes::refresh::refresh_token))
        // make sure these routes are rate limited
        .layer(GovernorLayer {
            config: secure_governor.clone(),
        });

    // Routes that require the user to be logged in and bearing a JWT
    let requires_token = Router::new()
        .route("/create", post(routes::create::create_game))
        .route("/join/{game_id}", get(routes::join::join_game))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth,
        ));

    Router::new()
        .route("/", any(routes::health::health_check))
        .route("/list", get(routes::list::game_list))
        .merge(authorization_providers)
        .merge(requires_token)
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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap()
}
