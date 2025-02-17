mod channels;
mod client;
pub mod config;
mod player;
mod state;

use std::sync::{atomic::AtomicBool, Arc};

use common::event::server;
use config::Config;
use futures::FutureExt;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use state::{lobby, playing};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

type GameData = Arc<Mutex<common::data::GameData>>;
type Channels = Arc<channels::Channels>;

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
        let channels = Arc::new(channels::Channels::start());

        let run = state::runner();

        let (disconnects, disconnect_handle) = client::disconnect(Arc::clone(&channels));

        let (connect_enabled, connect_handle) = client::connect(
            self.config.clone(),
            Arc::clone(&data),
            Arc::clone(&channels),
            disconnects,
        )
        .await;

        let mut state = State::Lobby;
        let mut data = Data {
            config: self.config.clone(),
            data,
            channels: channels.clone(),
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
                        match res {
                            Ok(res) => (state, data) = res,
                            Err(_) => break,
                        }
                    }
                }
            }
        };

        tokio::select! {
            _ = game => {
                close(channels).await;
            }
            _ = token.cancelled() => {
                close(channels).await;
                connect_handle.abort();
                disconnect_handle.abort();
            }
        }
    }
}

async fn close(channels: Channels) {
    channels.broadcast_event(server::Event::ServerClosing).await;
    channels.broadcast_command(player::Command::Close).await;
}
