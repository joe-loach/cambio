use std::{
    ops::ControlFlow,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use common::{
    data::Stage,
    event::{client, server},
};
use tokio::sync::mpsc;
use tracing::{info, trace, warn};

use crate::{
    channels::ClientEvents,
    config::{self, Config},
    player::Command,
    Channels, GameData, State,
};

pub struct Data {
    pub config: Config,
    pub data: GameData,
    pub channels: Channels,
    pub events: mpsc::Receiver<ClientEvents>,
    pub connect_enabled: Arc<AtomicBool>,
}

pub async fn lobby(
    Data {
        config,
        mut data,
        channels,
        mut events,
        connect_enabled,
    }: Data,
) -> State {
    trace!("enter lobby");
    super::notify_stage_change(Stage::Lobby, &channels, &mut data);

    let mut server_events = channels.lock().subscribe_to_all();
    connect_enabled.store(true, Ordering::Relaxed);

    'waiting: loop {
        info!(
            "waiting for clients ({}/{})",
            data.lock().player_count(),
            config::MAX_PLAYER_COUNT
        );

        'interrupt: loop {
            let can_start = data.lock().player_count() >= config::MIN_PLAYER_COUNT;

            tokio::select! {
                // request to start game
                Some((id, event)) = events.recv(), if can_start => {
                    if try_start_game(id, event, &data).is_break() {
                        break 'waiting
                    }
                }
                // events
                Ok(Command::Event(event)) = server_events.recv() => {
                    match event {
                        // update the logged player count when
                        // someone joins or leaves
                        server::Event::Joined { .. }
                        | server::Event::Left { .. } => break 'interrupt,
                        _ => (),
                    }
                }
                // keep waiting!
                else => {}
            };
        }

        if data.lock().player_count() == config::MAX_PLAYER_COUNT {
            info!("max lobby capacity reached");
            break 'waiting;
        }
    }

    connect_enabled.store(false, Ordering::Relaxed);

    trace!("exiting lobby");
    State::Playing(super::playing::Data {
        config,
        data,
        channels,
        events,
    })
}

fn try_start_game(id: uuid::Uuid, event: client::Event, data: &GameData) -> ControlFlow<()> {
    if let client::Event::Start = event {
        if super::host_id(data).is_some_and(|host| host == id) {
            info!("host started game");
            return ControlFlow::Break(());
        }
    } else {
        warn!("player {id} in lobby gave another event {event:?} when expecting `Event::Start`");
    }

    ControlFlow::Continue(())
}
