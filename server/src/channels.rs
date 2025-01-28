use std::collections::HashMap;
use std::future::Future;

use common::event::{client, server};
use futures::future::JoinAll;
use tokio::sync::{broadcast, mpsc};
use tracing::trace;

use crate::config;
use crate::player::{self, Command};

pub type ClientEvents = (uuid::Uuid, client::Event);

pub struct Channels {
    all: broadcast::Sender<player::Command>,
    map: HashMap<uuid::Uuid, mpsc::Sender<player::Command>>,
    event_sender: mpsc::Sender<ClientEvents>,
}

impl Channels {
    pub fn new() -> (Self, mpsc::Receiver<ClientEvents>) {
        const CHANNEL_CAPACITY: usize = 32;

        let (all, _) = broadcast::channel(CHANNEL_CAPACITY);
        let (event_sender, event_recv) = mpsc::channel(CHANNEL_CAPACITY);

        let this = Self {
            all,
            map: HashMap::with_capacity(config::MIN_PLAYER_COUNT),
            event_sender,
        };

        (this, event_recv)
    }

    pub fn subscribe_to_all(&self) -> broadcast::Receiver<player::Command> {
        self.all.subscribe()
    }

    pub fn event_sender(&self) -> mpsc::Sender<(uuid::Uuid, client::Event)> {
        self.event_sender.clone()
    }

    pub fn insert(&mut self, id: uuid::Uuid, tx: mpsc::Sender<Command>) {
        self.map.insert(id, tx);
    }

    pub fn remove(&mut self, id: uuid::Uuid) {
        self.map.remove(&id);
    }

    pub fn broadcast(&self, event: server::Event) {
        self.send_all(player::Command::Event(event));
    }

    pub fn send_all(&self, command: player::Command) {
        let _ = self.all.send(command.clone());
        trace!("broadcasting command: `{command:?}`");
    }

    pub fn map_id<F>(&self, f: F) -> JoinAll<impl Future<Output = ()>>
    where
        F: Fn(uuid::Uuid) -> server::Event,
    {
        self.map
            .iter()
            .filter(|(_, sender)| !sender.is_closed())
            .map(|(id, sender)| {
                let sender = sender.clone();
                let id = *id;
                let event = f(id);
                async move {
                    trace!("sending event: `{event:?}` to {id}");
                    let _ = sender.send(player::Command::Event(event)).await;
                }
            })
            .collect::<JoinAll<_>>()
    }
}
