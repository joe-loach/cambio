use std::{
    ops::ControlFlow,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use common::event::client;
use tracing::{info, warn};

use crate::{channels::Connection, config, Channels, GameData};

pub async fn run(
    game_data: &mut GameData,
    channels: &Channels,
    connect_enabled: Arc<AtomicBool>,
) {
    connect_enabled.store(true, Ordering::Relaxed);

    let mut client_events = channels.incoming();
    let mut connects = channels.connections().subscribe();

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
                Ok((id, event)) = client_events.recv(), if can_start => {
                    if try_start_game(id, event, game_data).is_break() {
                        break 'waiting
                    }
                }
                // update the logged player count when someone joins or leaves
                Ok(conn @ (Connection::Disconnect(..) | Connection::Connect(..))) = connects.recv() => {
                    // when someone leaves during the lobby phase,
                    // we remove them from the game data
                    if let Connection::Disconnect(id) = conn {
                        game_data.lock().remove_player(id);
                    }
                    break 'interrupt
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
}

fn try_start_game(id: uuid::Uuid, event: client::Event, data: &GameData) -> ControlFlow<()> {
    if let client::Event::Start = event {
        if host_id(data).is_some_and(|host| host == id) {
            info!("host started game");
            return ControlFlow::Break(());
        }
    } else {
        warn!("player {id} in lobby gave another event {event:?} when expecting `Event::Start`");
    }

    ControlFlow::Continue(())
}

fn host_id(data: &GameData) -> Option<uuid::Uuid> {
    // TODO: make "host" a dedicated field so that the host can be changed
    data.lock().players().first().map(|p| p.id())
}
