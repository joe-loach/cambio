pub mod lobby;
pub mod playing;

use common::{data::Stage, event::server};

use crate::{Channels, GameData};

pub fn notify_stage_change(stage: Stage, channels: &Channels, data: &mut GameData) {
    data.lock().stage = stage;
    channels.lock().broadcast(server::Event::StageChange(stage));
}

pub fn host_id(data: &GameData) -> Option<uuid::Uuid> {
    data.lock().players().first().map(|p| p.id())
}
