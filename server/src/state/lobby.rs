use std::{ops::ControlFlow, sync::atomic::Ordering};

use common::{
    data::Stage,
    event::{client, server},
};
use tracing::{info, trace, warn};

use crate::{config, player::Command, Data, GameData, State};

pub async fn lobby(mut data: Data) -> (State, Data) {
    let Data {
        config: _,
        data: ref mut game_data,
        ref channels,
        ref mut events,
        ref connect_enabled,
    } = data;

    trace!("enter lobby");
    super::notify_stage_change(Stage::Lobby, channels, game_data);

    let mut server_events = channels.lock().subscribe_to_all();
    connect_enabled.store(true, Ordering::Relaxed);

    'waiting: loop {
        info!(
            "waiting for clients ({}/{})",
            game_data.lock().player_count(),
            config::MAX_PLAYER_COUNT
        );

        'interrupt: loop {
            let can_start = game_data.lock().player_count() >= config::MIN_PLAYER_COUNT;

            tokio::select! {
                // request to start game
                Some((id, event)) = events.recv(), if can_start => {
                    if try_start_game(id, event, game_data).is_break() {
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

        if game_data.lock().player_count() == config::MAX_PLAYER_COUNT {
            info!("max lobby capacity reached");
            break 'waiting;
        }
    }

    connect_enabled.store(false, Ordering::Relaxed);

    trace!("exiting lobby");
    (State::Playing, data)
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
