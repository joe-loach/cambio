use std::{collections::HashMap, sync::Arc};

use common::event::{client, server};
use futures::future::JoinAll;
use parking_lot::RwLock;
use tokio::sync::{broadcast, mpsc, oneshot};
use tracing::trace;

use crate::player;
use crate::{config, player::CloseReason};

pub type ClientEvents = (uuid::Uuid, client::Event);

#[derive(Clone)]
pub enum Connection {
    Disconnect(uuid::Uuid, CloseReason),
    Connect(uuid::Uuid),
}

struct Process {
    kind: ProcessKind,
    sync: oneshot::Sender<()>,
}

enum ProcessKind {
    Insert {
        id: uuid::Uuid,
        tx: mpsc::Sender<player::Command>,
    },
    Remove {
        id: uuid::Uuid,
    },
    Send(SendTo),
}

enum SendTo {
    All(Box<dyn Fn(uuid::Uuid) -> player::Command + Send>),
    One(player::Command, uuid::Uuid),
}

pub struct Channels {
    out: mpsc::Sender<Process>,
    incoming: broadcast::Sender<ClientEvents>,
    connections: broadcast::Sender<Connection>,
}

impl Channels {
    pub fn start() -> Self {
        const PROCESSING_CAPACITY: usize = 128;

        let (out, out_rx) = mpsc::channel(PROCESSING_CAPACITY);
        let (connections, _) = broadcast::channel(PROCESSING_CAPACITY);

        tokio::spawn(send_aggregator(out_rx));

        let (incoming, _) = broadcast::channel(PROCESSING_CAPACITY);

        Self {
            out,
            incoming,
            connections,
        }
    }

    /// Register a new player channel, returning the sender for Client events.
    pub async fn register(
        &self,
        id: uuid::Uuid,
        tx: mpsc::Sender<player::Command>,
    ) -> broadcast::Sender<ClientEvents> {
        let (sync, finished) = oneshot::channel();
        let _ = self
            .out
            .send(Process {
                kind: ProcessKind::Insert { id, tx },
                sync,
            })
            .await;
        finished.await.expect("failed to sync");
        self.incoming.clone()
    }
}

impl Channels {
    pub async fn remove(&self, id: uuid::Uuid) {
        let (sync, finished) = oneshot::channel();
        let _ = self
            .out
            .send(Process {
                kind: ProcessKind::Remove { id },
                sync,
            })
            .await;
        finished.await.expect("failed to sync");
    }

    pub async fn send(&self, command: player::Command, id: uuid::Uuid) {
        let (sync, finished) = oneshot::channel();
        let _ = self
            .out
            .send(Process {
                kind: ProcessKind::Send(SendTo::One(command, id)),
                sync,
            })
            .await;
        finished.await.expect("failed to sync");
    }

    pub async fn broadcast_event(&self, event: server::Event) {
        let _ = self.broadcast_command(player::Command::Event(event)).await;
    }

    pub async fn broadcast_command(&self, command: player::Command) {
        let (sync, finished) = oneshot::channel();
        let _ = self
            .out
            .send(Process {
                kind: ProcessKind::Send(SendTo::All(Box::new(move |_| command.clone()))),
                sync,
            })
            .await;
        finished.await.expect("failed to sync");
    }

    pub async fn broadcast_map<F>(&self, f: F)
    where
        F: Fn(uuid::Uuid) -> player::Command + Send + 'static,
    {
        let (sync, finished) = oneshot::channel();
        let _ = self
            .out
            .send(Process {
                kind: ProcessKind::Send(SendTo::All(Box::new(f))),
                sync,
            })
            .await;
        finished.await.expect("failed to sync");
    }
}

impl Channels {
    /// Client connections and disconnections.
    pub fn connections(&self) -> broadcast::Sender<Connection> {
        self.connections.clone()
    }

    /// Incoming events from Client.
    pub fn incoming(&self) -> broadcast::Receiver<ClientEvents> {
        self.incoming.subscribe()
    }
}

/// Maintains a map of streams corresponding to each player.
/// Messages then can be sent to All or One player.
///
/// This allows `Channels` to be immutable.
async fn send_aggregator(mut out_rx: mpsc::Receiver<Process>) {
    // use a RwLock in the hopes that concurrent reads are performed more often than writes
    let map = Arc::new(RwLock::new(HashMap::with_capacity(
        config::MIN_PLAYER_COUNT,
    )));

    while let Some(proc) = out_rx.recv().await {
        let sync = proc.sync;
        match proc.kind {
            ProcessKind::Send(send_to) => {
                let map = Arc::clone(&map);
                tokio::spawn(async move {
                    handle_send(map, send_to).await;
                    let _ = sync.send(());
                });
            }
            ProcessKind::Insert { id, tx } => {
                map.write().insert(id, tx);
                let _ = sync.send(());
            }
            ProcessKind::Remove { id } => {
                map.write().remove(&id);
                let _ = sync.send(());
            }
        }
    }
}

async fn handle_send(
    map: Arc<RwLock<HashMap<uuid::Uuid, mpsc::Sender<player::Command>>>>,
    send: SendTo,
) {
    match send {
        SendTo::All(cmd) => {
            let join = map
                .read()
                .iter()
                .filter(|(_, sender)| !sender.is_closed())
                .map(|(id, sender)| {
                    let sender = sender.clone();
                    let id = *id;
                    let cmd = cmd(id);
                    async move {
                        trace!("sending cmd: `{cmd:?}` to {id}");
                        let _ = sender.send(cmd).await;
                    }
                })
                .collect::<JoinAll<_>>();
            join.await;
        }
        SendTo::One(cmd, id) => {
            let sender = map.read().get(&id).cloned();
            if let Some(sender) = sender {
                trace!("sending cmd: `{cmd:?}` to {id}");
                let _ = sender.send(cmd).await;
            }
        }
    }
}
