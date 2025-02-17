use std::{net::SocketAddr, sync::Arc};

use axum::{extract::State, Json};
use serde::Serialize;

use crate::{db::DbError, id::Id, models::game::Game, AppState};

#[derive(Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct Listing {
    pub(crate) id: Id,
    pub(crate) name: String,
    pub(crate) address: SocketAddr,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(serde::Deserialize))]
pub struct GameListResponse {
    pub(crate) game_listings: Vec<Listing>,
}

pub async fn game_list(
    State(state): State<Arc<AppState<'_>>>,
) -> Result<Json<GameListResponse>, DbError> {
    let r = state.db.read()?;

    let game_listings = r
        .scan()
        .primary::<Game>()?
        .all()?
        .filter_map(Result::ok)
        .filter(|g| g.is_public())
        .map(|g| Listing {
            id: g.id,
            name: g.info.name,
            address: g.info.server_addr,
        })
        .collect();

    Ok(Json(GameListResponse { game_listings }))
}
