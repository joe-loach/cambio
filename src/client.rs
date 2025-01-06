use futures::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpStream, ToSocketAddrs},
    select,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::{server, stream};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    Start,
    Snap,
    Decision,
    Continue,
    Leave,
}

pub struct GameClient {
    read: stream::Read<server::Event>,
    write: stream::Write<Event>,
}

impl GameClient {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> Self {
        let stream = TcpStream::connect(addr).await.unwrap();

        let (read, write) = stream::split::<server::Event, Event>(stream);

        Self { read, write }
    }

    pub async fn start(mut self, token: CancellationToken) {
        select! {
            _ = self.game_loop() => {}
            _ = token.cancelled() => {
                info!("leaving server");
                self.write.send(Event::Leave).await.unwrap();
            }
        }
    }

    async fn game_loop(&mut self) {
        let Some(server::Event::Joined { slot, .. }) = self.read.try_next().await.unwrap() else {
            error!("failed to join");
            return;
        };

        info!("joined lobby");

        let mut turn = usize::MAX;

        while let Some(msg) = self.read.try_next().await.unwrap() {
            println!("GOT: {:?}", msg);

            match msg {
                server::Event::Joined { player_count: capacity, .. } => {
                    if capacity >= 2 {
                        let _ = self.write.send(Event::Start).await;
                    }
                }
                server::Event::TurnStart { slot, uuid: _ } => {
                    turn = slot;
                }
                server::Event::WaitingForDecision if turn == slot => {
                    self.write.send(Event::Decision).await.unwrap();
                }
                server::Event::WaitingForSnap if turn != slot => {
                    self.write.send(Event::Snap).await.unwrap();
                }
                server::Event::ServerClosing => {
                    break;
                }
                _ => (),
            }
        }
    }
}
