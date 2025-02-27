use std::ops::ControlFlow;

use common::event::{client, server};
use common::stream;
use futures::SinkExt;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tracing::error;

use crate::{Channels, GameData};

pub struct PlayerConn {
    pub read: stream::Read<client::Event>,
    pub write: stream::Write<server::Event>,
}

impl PlayerConn {
    pub fn from(socket: TcpStream) -> Self {
        let (read, write) = stream::split(socket);
        Self { read, write }
    }
}

pub async fn spawn(
    data: GameData,
    channels: &Channels,
    id: uuid::Uuid,
    mut conn: PlayerConn,
) -> oneshot::Receiver<()> {
    const EVENT_CAPACITY: usize = 32;

    let (tx, rx) = mpsc::channel(EVENT_CAPACITY);

    let events = channels.register(id, tx).await;

    // turn channels into streams
    let mut server_events = Box::pin(ReceiverStream::new(rx));

    let (closing_rx, closing_tx) = oneshot::channel();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = server_events.next() => {
                    match res {
                        Some(event) => {
                            conn.write.send(event).await.expect("failed to send event");
                        }
                        None => {
                            break;
                        }
                    }
                }
                res = conn.read.next() => {
                    match res {
                        Some(Ok(event)) => {
                            match try_handle_early(&data, &mut conn, event).await {
                                Ok(ControlFlow::Continue(())) => (),
                                Ok(ControlFlow::Break(())) => break,
                                Err(e) => {
                                    // couldn't handle this event early,
                                    // forward it on to any listeners
                                    let _ = events.send((id, e));
                                }
                            }
                        }
                        Some(Err(e)) => {
                            error!("error in client ({id}): `{e}`");
                            break;
                        }
                        // client disconnected
                        None => {
                            break;
                        }
                    }
                }
            }
        }

        let _ = closing_rx.send(());
    });

    closing_tx
}

async fn try_handle_early(
    data: &GameData,
    conn: &mut PlayerConn,
    event: client::Event,
) -> Result<ControlFlow<()>, client::Event> {
    let res = match event {
        client::Event::Leave => ControlFlow::Break(()),
        client::Event::GetLobbyInfo => {
            let player_count = data.lock().player_count();

            let _ = conn
                .write
                .send(server::Event::LobbyInfo { player_count })
                .await;

            ControlFlow::Continue(())
        }
        e => return Err(e),
    };

    Ok(res)
}
