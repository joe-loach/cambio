mod channels;
mod client;
pub mod config;
mod player;
mod state;

use std::sync::Arc;

use common::event::server;
use config::Config;
use parking_lot::Mutex;
use player::CloseReason;
use state::{lobby, playing};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

type GameData = Arc<Mutex<common::data::GameData>>;
type Channels = Arc<Mutex<channels::Channels>>;
type Leave = (uuid::Uuid, CloseReason);

pub enum State {
    Lobby(lobby::Data),
    Playing(playing::Data),
    Exit,
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

        let (connect_enabled, connect_handle) = client::connect(
            self.config.clone(),
            Arc::clone(&data),
            Arc::clone(&channels),
            connect,
        )
        .await;

        let disconnect_handle =
            client::disconnect(Arc::clone(&data), Arc::clone(&channels), disconnect);

        let start = State::Lobby(state::lobby::Data {
            config: self.config.clone(),
            data,
            channels: channels.clone(),
            events,
            connect_enabled,
        });

        let game = async move {
            let mut state = start;
            loop {
                state = match state {
                    State::Lobby(data) => lobby::lobby(data).await,
                    State::Playing(data) => playing::playing(data).await,
                    State::Exit => return,
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
