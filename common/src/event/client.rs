use serde::{Deserialize, Serialize};

use crate::decisions::Decision;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Start,
    Snap,
    Decision(Decision),
    Continue,
    Leave,
}
