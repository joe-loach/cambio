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
use tracing::info;

use crate::{
    config::Config,
    player::{self, PlayerConn},
    Channels, GameData, Leave,
};

pub async fn connect(
    config: Config,
    mut data: GameData,
    mut channels: Channels,
    leaving: mpsc::Sender<Leave>,
) -> (Arc<AtomicBool>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], config.server_port)))
        .await
        .expect("failed to create server port");

    info!("listening on {:?}", listener.local_addr().ok());

    let enabled = Arc::new(AtomicBool::new(false));

    let handle = tokio::spawn({
        let enabled = Arc::clone(&enabled);
        async move {
            loop {
                let accepting = enabled.load(Ordering::Relaxed);
                tokio::select! {
                    Ok((stream, addr)) = listener.accept() => {
                        if accepting {
                            setup_client(
                                stream,
                                addr,
                                &mut data,
                                &mut channels,
                                leaving.clone()
                            )
                            .await;
                        } else {
                            drop(stream);
                        }
                    }
                }
            }
        }
    });

    (enabled, handle)
}

pub fn disconnect(
    data: GameData,
    channels: Channels,
    mut leaving: mpsc::Receiver<Leave>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Some((id, _)) = leaving.recv().await {
            info!("client {id} has left");
            let mut chan = channels.lock();
            let mut data = data.lock();
            data.remove_player(id);
            chan.broadcast(server::Event::Left {
                uuid: id,
                player_count: data.player_count(),
            });
            chan.remove(id);
        }
    })
}

async fn setup_client(
    stream: TcpStream,
    addr: SocketAddr,
    data: &mut GameData,
    channels: &mut Channels,
    leaving: mpsc::Sender<Leave>,
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

    // let everyone else know the player shutsdown
    tokio::spawn(async move {
        if let Ok(reason) = left.await {
            let _ = leaving.send((player_id, reason)).await;
        }
    });

    // let everyone know someone has joined
    channels.lock().broadcast(server::Event::Joined {
        uuid: player_id,
        player_count,
    });
}
