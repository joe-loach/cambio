use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Start,
    Snap,
    Decision,
    Continue,
    Leave,
}
