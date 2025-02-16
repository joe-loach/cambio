use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use crate::{
    db::DbError,
    models::game::{self, Game},
    AppState,
};

#[derive(Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct GameListResponse {
    pub(crate) game_info: Vec<game::GameInfo>,
}

pub async fn game_list(
    State(state): State<Arc<AppState<'_>>>,
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
