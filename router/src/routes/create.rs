use std::{net::SocketAddr, sync::Arc};

use axum::{extract::State, response::{IntoResponse, Response}, Json};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    db::{Db, DbError},
    id::Id,
    models::game::{self, Game},
    AppState,
};

#[derive(Deserialize)]
pub struct CreateGameRequest {
    name: String,
    visibility: game::Visibility,
    server_addr: SocketAddr,
}

#[derive(Serialize, Deserialize)]
pub struct CreateGameResponse {
    pub id: Id,
}

#[derive(Debug, Error)]
pub enum CreateGameError {
    #[error(transparent)]
    Db(#[from] DbError),
}

pub async fn create_game(
    State(state): State<Arc<AppState<'_>>>,
    Json(CreateGameRequest {
        name,
        visibility,
        server_addr,
    }): Json<CreateGameRequest>,
) -> Result<Json<CreateGameResponse>, CreateGameError> {
    let game_id = unique_game_id(&state.db)?;

    let new_game = Game::new(
        game_id.clone(),
        visibility,
        game::GameInfo { name, server_addr },
    );

    let rw = state.db.read_write()?;
    rw.insert(new_game)?;
    rw.commit()?;

    Ok(Json(CreateGameResponse { id: game_id }))
}

fn unique_game_id(db: &Db) -> Result<Id, CreateGameError> {
    loop {
        let id = Id::new();

        // make sure the id doesn't exist
        // TODO: improve this, the collision rate should be so low that it shouldn't matter
        let r = db.read()?;
        if r.get().primary::<Game>(id.clone())?.is_none() {
            return Ok(id);
        }
    }
}

impl IntoResponse for CreateGameError {
    fn into_response(self) -> Response {
        match self {
            CreateGameError::Db(db_error) => db_error.into_response(),
        }
    }
}
