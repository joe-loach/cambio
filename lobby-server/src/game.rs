use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::Id;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Game {
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
