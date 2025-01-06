use dashmap::DashMap;
use tokio::sync::{broadcast, mpsc};
use tracing::instrument;

use crate::{client, server::config};

use super::{
    player::{self, Command},
    Event,
};

pub struct Connections {
    all: broadcast::Sender<player::Command>,
    pub(crate) map: DashMap<uuid::Uuid, mpsc::Sender<player::Command>>,
    event_sender: mpsc::Sender<(uuid::Uuid, client::Event)>,
    events: mpsc::Receiver<(uuid::Uuid, client::Event)>,
}

impl Connections {
    pub fn new() -> Self {
        const CHANNEL_CAPACITY: usize = 32;

        let (all, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (event_sender, events) = mpsc::channel(CHANNEL_CAPACITY);

        Self {
            all,
            map: DashMap::with_capacity(config::MIN_PLAYER_COUNT),
            event_sender,
            events,
        }
    }

    pub fn subscribe_to_all(&self) -> broadcast::Receiver<player::Command> {
        self.all.subscribe()
    }

    pub fn event_sender(&self) -> mpsc::Sender<(uuid::Uuid, client::Event)> {
        self.event_sender.clone()
    }

    pub fn events(&mut self) -> &mut mpsc::Receiver<(uuid::Uuid, client::Event)> {
        &mut self.events
    }

    pub fn insert(&mut self, id: uuid::Uuid, tx: mpsc::Sender<Command>) {
        self.map.insert(id, tx);
    }

    pub fn broadcast(&self, event: Event) {
        self.send_all(player::Command::Event(event));
    }

    #[instrument(level = "trace", skip(self))]
    pub fn send_all(&self, command: player::Command) {
        let _ = self.all.send(command);
    }

    #[instrument(level = "trace", skip(self, f))]
    pub async fn send_map<F>(&self, f: F)
    where
        F: Fn(uuid::Uuid) -> Event,
    {
        for pair in &self.map {
            let event = f(*pair.key());
            pair.value()
                .send(player::Command::Event(event))
                .await
                .expect("channel closed");
        }
    }
}

impl Default for Connections {
    fn default() -> Self {
        Self::new()
    }
}
