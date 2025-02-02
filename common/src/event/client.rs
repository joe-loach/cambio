use serde::{Deserialize, Serialize};

use crate::decisions::Decision;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Join(Join),
    GetLobbyInfo,
    Start,
    Snap,
    Decision(Decision),
    Continue,
    Leave,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Join {
    New,
    Existing(uuid::Uuid),
}
