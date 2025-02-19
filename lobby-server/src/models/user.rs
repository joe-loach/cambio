use native_db::*;
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[native_model(id = 2, version = 1)]
#[native_db]
pub struct User {
    #[primary_key]
    pub(crate) name: String,
    pub(crate) password: String,
}
