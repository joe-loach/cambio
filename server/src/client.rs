use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use common::{
    data::PlayerData,
    event::{client, server},
};
use futures::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tracing::{error, info, trace, warn};

use crate::{
    channels::Connection,
    config::Config,
    player::{self, PlayerConn},
    Channels, GameData,
};

pub async fn connect(
    config: Config,
    mut data: GameData,
    channels: Channels,
    disconnects: Disconnects,
) -> (Arc<AtomicBool>, tokio::task::AbortHandle) {
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], config.server_port)))
        .await
        .expect("failed to create server port");

    info!("listening on {:?}", listener.local_addr().ok());

    let enabled = Arc::new(AtomicBool::new(false));

    let task = tokio::spawn({
        let enabled = Arc::clone(&enabled);
        async move {
            while let Ok((stream, addr)) = listener.accept().await {
                let accepting = enabled.load(Ordering::Relaxed);
                if accepting {
                    try_connect(stream, addr, &mut data, &channels, disconnects.clone()).await;
                } else {
                    trace!("not accepting, dropped {addr}");
                    drop(stream);
                }
            }
        }
    });

    (enabled, task.abort_handle())
}

#[derive(Clone)]
pub struct Disconnects(mpsc::Sender<uuid::Uuid>);

pub fn disconnect(channels: Channels) -> (Disconnects, tokio::task::AbortHandle) {
    let (tx, mut rx) = mpsc::channel(16);

    let task = tokio::spawn(async move {
        while let Some(id) = rx.recv().await {
            info!("client {id} has left");
            channels.remove(id).await;
            channels.broadcast_event(server::Event::Left { id }).await;
            // let subscribers know theres been a disconnection
            let _ = channels.connections().send(Connection::Disconnect(id));
        }
    });

    (Disconnects(tx), task.abort_handle())
}

async fn try_connect(
    stream: TcpStream,
    addr: SocketAddr,
    data: &mut GameData,
    channels: &Channels,
    disconnects: Disconnects,
) {
    info!("connection from {addr}");

    let mut connection = PlayerConn::from(stream);

    // retrieve id of player
    let Some(id) = id_handshake(data, &mut connection).await else {
        return;
    };

    // let other clients know someone has joined
    trace!("sending join");
    channels.broadcast_event(server::Event::Joined { id }).await;

    // let subscribers know theres a new connection
    let _ = channels.connections().send(Connection::Connect(id));

    // spawn a player task
    let left = player::spawn(Arc::clone(data), channels, id, connection).await;

    // let the disconnect handler know
    tokio::spawn(async move {
        if left.await.is_ok() {
            let _ = disconnects.0.send(id).await;
        }
    });

    // signal to the player that they can join the event loop
    channels.send(server::Event::Enter, id).await;
}

async fn id_handshake(data: &mut GameData, connection: &mut PlayerConn) -> Option<uuid::Uuid> {
    let Some(Ok(client::Event::Join(join))) = connection.read.next().await else {
        warn!("connection refused as client never requested to join");
        return None;
    };

    let id = retrieve_or_create_id(data, join);

    // assign the player their id
    if let Err(e) = connection.write.send(server::Event::AssignId { id }).await {
        error!("failed to assign id to client: {e}");
        return None;
    }

    Some(id)
}

fn retrieve_or_create_id(data: &mut GameData, join: client::Join) -> uuid::Uuid {
    let create_new = || {
        let player = PlayerData::new();
        let id = player.id();

        data.lock().try_add_player(player);

        id
    };

    match join {
        client::Join::New => create_new(),
        client::Join::Existing(id) if !data.lock().exists(id) => {
            warn!("client tried to connect with invalid id");
            create_new()
        }
        client::Join::Existing(id) => {
            debug_assert!(data.lock().exists(id));
            id
        }
    }
}
