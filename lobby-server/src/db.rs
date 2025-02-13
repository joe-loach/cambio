use std::{env, sync::LazyLock};

use native_db::{Builder, Database, Models, ToInput, ToKey};

/// Add Models that can be "understood" by the database here.
static MODELS: LazyLock<Models> = LazyLock::new(|| {
    let mut models = Models::new();
    models.define::<crate::models::game::Game>().unwrap();
    models.define::<crate::models::user::User>().unwrap();
    models
});

pub struct Db<'db> {
    inner: Database<'db>,
}

pub fn establish_connection<'db>() -> anyhow::Result<Db<'db>> {
    let path = env::var("DATABASE_PATH")?;
    let db = Builder::new().create(&MODELS, path)?;

    Ok(Db { inner: db })
}

impl Db<'_> {
    pub fn read_write(&self) -> Result<RwTransaction<'_>> {
        Ok(RwTransaction(self.inner.rw_transaction()?))
    }

    pub fn read(&self) -> Result<RTransaction<'_>> {
        Ok(RTransaction(self.inner.r_transaction()?))
    }
}

pub struct RwTransaction<'db>(native_db::transaction::RwTransaction<'db>);

impl RwTransaction<'_> {
    pub fn insert<T: ToInput>(&self, item: T) -> Result<()> {
        Ok(self.0.insert(item)?)
    }

    pub fn commit(self) -> Result<()> {
        Ok(self.0.commit()?)
    }
}

pub struct RTransaction<'db>(native_db::transaction::RTransaction<'db>);

impl<'db> RTransaction<'db> {
    pub fn get(&self) -> RGet<'db, '_> {
        RGet(self.0.get())
    }

    pub fn scan(&self) -> native_db::transaction::query::RScan<'_, '_> {
        self.0.scan()
    }
}

pub struct RGet<'db, 'txn>(native_db::transaction::query::RGet<'db, 'txn>);

impl RGet<'_, '_> {
    pub fn primary<T: ToInput>(&self, key: impl ToKey) -> Result<Option<T>> {
        Ok(self.0.primary::<T>(key)?)
    }
}

use axum::{http::StatusCode, response::IntoResponse};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, Error)]
#[error("Database error: {0}")]
pub struct DbError(#[from] native_db::db_type::Error);

impl IntoResponse for DbError {
    fn into_response(self) -> axum::response::Response {
        tracing::error!(error = %self.0);

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Something went wrong".to_owned(),
        )
            .into_response()
    }
}
