use common::{
    event::{client, server},
    stream,
};
use futures::prelude::*;
use tokio::{
    net::{TcpStream, ToSocketAddrs},
    select,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct GameClient {
    read: stream::Read<server::Event>,
    write: stream::Write<client::Event>,
}

impl GameClient {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> Self {
        let stream = TcpStream::connect(addr).await.unwrap();

        let (read, write) = stream::split(stream);

        Self { read, write }
    }

    pub async fn start(mut self, token: CancellationToken) {
        select! {
            _ = self.game_loop() => {}
            _ = token.cancelled() => {
                info!("leaving server");
                self.write.send(client::Event::Leave).await.unwrap();
            }
        }
    }

    async fn game_loop(&mut self) {
        let Some(server::Event::Joined { uuid, .. }) = self.read.try_next().await.unwrap() else {
            error!("failed to join");
            return;
        };

        info!("joined lobby");

        let mut turn = uuid;

        while let Some(msg) = self.read.try_next().await.unwrap() {
            println!("GOT: {:?}", msg);

            match msg {
                server::Event::Joined {
                    player_count: capacity,
                    ..
                } => {
                    if capacity >= 2 {
                        let _ = self.write.send(client::Event::Start).await;
                    }
                }
                server::Event::TurnStart { uuid, .. } => {
                    turn = uuid;
                }
                server::Event::WaitingForDecision if turn == uuid => {
                    self.write.send(client::Event::Decision).await.unwrap();
                }
                server::Event::WaitingForSnap if turn != uuid => {
                    self.write.send(client::Event::Snap).await.unwrap();
                }
                server::Event::ServerClosing => {
                    break;
                }
                _ => (),
            }
        }
    }
}
