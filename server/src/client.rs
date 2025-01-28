use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use common::{data::PlayerData, event::server};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tracing::{info, trace};

use crate::{
    channels::Connection,
    config::Config,
    player::{self, CloseReason, PlayerConn},
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
                    setup_client(stream, addr, &mut data, &channels, disconnects.clone()).await;
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
pub struct Disconnects(mpsc::Sender<(uuid::Uuid, CloseReason)>);

pub fn disconnect(data: GameData, channels: Channels) -> (Disconnects, tokio::task::AbortHandle) {
    let (tx, mut rx) = mpsc::channel(16);

    let task = tokio::spawn(async move {
        while let Some((id, reason)) = rx.recv().await {
            info!("client {id} has left");
            let player_count = {
                let mut data = data.lock();
                data.remove_player(id);
                data.player_count()
            };
            channels.remove(id).await;
            channels
                .broadcast_event(server::Event::Left {
                    uuid: id,
                    player_count,
                })
                .await;
            // let subscribers know theres been a disconnection
            let _ = channels
                .connections()
                .send(Connection::Disconnect(id, reason));
        }
    });

    (Disconnects(tx), task.abort_handle())
}

async fn setup_client(
    stream: TcpStream,
    addr: SocketAddr,
    data: &mut GameData,
    channels: &Channels,
    disconnects: Disconnects,
) {
    info!("new connection from {addr}");
    let player = PlayerData::new();
    let player_id = player.id();

    let player_count = {
        let mut data = data.lock();

        data.add_player(player);
        data.player_count()
    };

    // spawn a player task
    let left = player::spawn(channels, player_id, PlayerConn::from(stream)).await;

    // let the disconnect handler know
    tokio::spawn(async move {
        if let Ok(reason) = left.await {
            let _ = disconnects.0.send((player_id, reason)).await;
        }
    });

    // let everyone know someone has joined
    channels
        .broadcast_event(server::Event::Joined {
            uuid: player_id,
            player_count,
        })
        .await;
    // let subscribers know theres a new connection
    let _ = channels.connections().send(Connection::Connect(player_id));
}
