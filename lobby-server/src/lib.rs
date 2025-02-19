pub mod db;
pub mod error;
pub mod id;
pub mod models;
pub mod routes;
pub mod token;

pub struct AppState<'a> {
    pub db: db::Db<'a>,
}
