mod channels;
mod client;
pub mod config;
mod player;
mod state;

use std::sync::{atomic::AtomicBool, Arc};

use channels::ClientEvents;
use common::event::server;
use config::Config;
use futures::FutureExt;
use parking_lot::Mutex;
use player::CloseReason;
use serde::{Deserialize, Serialize};
use state::{lobby, playing};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

type GameData = Arc<Mutex<common::data::GameData>>;
type Channels = Arc<Mutex<channels::Channels>>;
type Leave = (uuid::Uuid, CloseReason);

#[derive(Serialize, Deserialize)]
pub enum State {
    Lobby,
    Playing,
    Suspend,
    Exit,
}

pub struct Data {
    pub config: Config,
    pub data: GameData,
    pub channels: Channels,
    pub events: mpsc::Receiver<ClientEvents>,
    pub connect_enabled: Arc<AtomicBool>,
}

pub struct GameServer {
    config: Config,
}

impl GameServer {
    pub fn from_config() -> Self {
        let config = match config::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("error loading config: {e}");
                info!("using default config");
                Config::default()
            }
        };

        GameServer { config }
    }

    pub async fn run(&self, token: CancellationToken) {
        let data = Arc::new(Mutex::new(Default::default()));

        let (channels, events) = channels::Channels::new();
        let channels = Arc::new(Mutex::new(channels));
        let (connect, disconnect) = mpsc::channel(16);

        let run = state::runner();

        let (connect_enabled, connect_handle) = client::connect(
            self.config.clone(),
            Arc::clone(&data),
            Arc::clone(&channels),
            connect,
        )
        .await;

        let disconnect_handle =
            client::disconnect(Arc::clone(&data), Arc::clone(&channels), disconnect);

        let mut state = State::Lobby;
        let mut data = Data {
            config: self.config.clone(),
            data,
            channels: channels.clone(),
            events,
            connect_enabled,
        };

        let game = async move {
            loop {
                let task = match state {
                    State::Lobby => lobby::lobby(data).boxed(),
                    State::Playing => playing::playing(data).boxed(),
                    State::Suspend => return,
                    State::Exit => return,
                };

                tokio::select! {
                    res = state::process(&run, task) => {
                        (state, data) = res;
                    }
                }
            }
        };

        tokio::select! {
            _ = game => {
                close(channels);
            }
            _ = token.cancelled() => {
                close(channels);
                connect_handle.abort();
                disconnect_handle.abort();
            }
        }
    }
}

fn close(channels: Channels) {
    let channels = channels.lock();
    channels.broadcast(server::Event::ServerClosing);
    channels.send_all(player::Command::Close);
}
