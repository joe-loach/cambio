use std::net::SocketAddr;

use native_db::*;
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};

use crate::Id;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[native_model(id = 1, version = 1)]
#[native_db]
pub(crate) struct Game {
    #[primary_key]
    pub(crate) id: Id,
    visibility: Visibility,
    pub(crate) info: GameInfo,
}

impl Game {
    pub(crate) fn new(id: Id, visibility: Visibility, info: GameInfo) -> Self {
        Self {
            id,
            visibility,
            info,
        }
    }

    /// Returns `true` if the visibility is [`Public`].
    ///
    /// [`Public`]: Visibility::Public
    pub fn is_public(&self) -> bool {
        self.visibility.is_public()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Visibility {
    Public,
    Private,
}

impl Visibility {
    /// Returns `true` if the visibility is [`Public`].
    ///
    /// [`Public`]: Visibility::Public
    #[must_use]
    pub(crate) fn is_public(&self) -> bool {
        matches!(self, Self::Public)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GameInfo {
    pub(crate) name: String,
    pub(crate) server_addr: SocketAddr,
}
