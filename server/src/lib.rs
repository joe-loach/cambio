mod channels;
mod client;
pub mod config;
mod game;
mod lobby;
mod player;

use std::sync::Arc;

use common::event::server::{self};
use config::Config;
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

type GameData = Arc<Mutex<common::data::GameData>>;
type Channels = Arc<channels::Channels>;

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
        let mut data = Arc::new(Mutex::new(Default::default()));
        let channels = Arc::new(channels::Channels::start());

        let (disconnects, disconnect_handle) = client::disconnect(Arc::clone(&channels));
        let (connect_enabled, connect_handle) = client::connect(
            self.config.clone(),
            Arc::clone(&data),
            Arc::clone(&channels),
            disconnects,
        )
        .await;

        let mut game = game::Game::new();

        tokio::select! {
            _ = async {
                lobby::run(&mut data, &channels, connect_enabled.clone()).await;
                game::run(&mut game, &data, &channels).await;
            } => {}
            _ = token.cancelled() => {
                let _ = game;
            }
        }

        channels.broadcast_event(server::Event::ServerClosing).await;
        connect_handle.abort();
        disconnect_handle.abort();
    }
}
