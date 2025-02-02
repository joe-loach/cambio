use common::{
    event::{
        client::{self, Event},
        server,
    },
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
        self.write
            .send(Event::Join(client::Join::New))
            .await
            .unwrap();

        let Some(server::Event::AssignId { id }) = self.read.try_next().await.unwrap() else {
            error!("failed to get ID");
            return;
        };

        let read = self.read.try_next().await.unwrap();
        let Some(server::Event::Enter) = read else {
            error!("never received enter: {read:?}");
            return;
        };

        info!("entering event loop");

        async fn request_lobby_info(writer: &mut stream::Write<client::Event>) {
            writer.send(Event::GetLobbyInfo).await.unwrap();
        }

        request_lobby_info(&mut self.write).await;

        let mut turn = id;
        let mut card_in_hand = None;

        while let Some(msg) = self.read.try_next().await.unwrap() {
            println!("GOT: {:?}", msg);

            match msg {
                server::Event::LobbyInfo { player_count } => {
                    if player_count >= 2 {
                        let _ = self.write.send(client::Event::Start).await;
                    }
                }
                server::Event::Joined { id: _ } => {
                    request_lobby_info(&mut self.write).await;
                }
                server::Event::Left { id: _ } => {
                    request_lobby_info(&mut self.write).await;
                }
                server::Event::TurnStart { id, .. } => {
                    turn = id;
                }
                server::Event::DrawCard(card) if turn == id => {
                    card_in_hand = Some(card);
                }
                server::Event::WaitingForDecision if turn == id => {
                    if let Some(card) = card_in_hand.take() {
                        let valid_decisions = common::decisions::valid_set(card).into_vec();
                        // TODO: let the user choose from the vector
                        let decision = *valid_decisions.first().unwrap();
                        self.write
                            .send(client::Event::Decision(decision))
                            .await
                            .unwrap();
                    }
                }
                server::Event::WaitingForSnap if turn != id => {
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
