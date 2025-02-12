use std::sync::Arc;

use axum::{Extension, Json};
use serde::Serialize;

use crate::{db::DbError, models::game::{self, Game}, AppState};

#[derive(Serialize)]
pub struct GameListResponse {
    game_info: Vec<game::GameInfo>,
}

pub async fn game_list(
    Extension(state): Extension<Arc<AppState<'_>>>,
) -> Result<Json<GameListResponse>, DbError> {
    let r = state.db.read()?;

    let game_info = r
        .scan()
        .primary::<Game>()?
        .all()?
        .filter_map(Result::ok)
        .filter(|g| g.is_public())
        .map(|g| g.info.clone())
        .collect();

    Ok(Json(GameListResponse { game_info }))
}
