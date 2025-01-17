use common::event::{client, server};
use common::stream;
use futures::SinkExt;
use tokio::sync::{mpsc, oneshot};
use tokio::{net::TcpStream, select};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};
use tracing::error;

use crate::Channels;

#[derive(Debug)]
pub enum CloseReason {
    Request,
    Exhausted,
}

#[derive(Debug, Clone)]
pub enum Command {
    Event(server::Event),
    Close,
}

pub async fn spawn(
    channels: &mut Channels,
    id: uuid::Uuid,
    mut conn: PlayerConn,
) -> oneshot::Receiver<CloseReason> {
    const COMMAND_CHANNEL_CAPACITY: usize = 32;
    
    let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);

    let (event_sender, broadcast) = {
        let mut chan = channels.lock();
        let event_sender = chan.event_sender();
        let broadcast = chan.subscribe_to_all();

        chan.insert(id, tx);
        
        (event_sender, broadcast)
    };

    // turn channels into streams
    let own = Box::pin(ReceiverStream::new(rx));
    let broadcast =
        Box::pin(BroadcastStream::new(broadcast).filter_map(|res| res.ok()));

    // combine commands from both sources
    let mut commands = own.merge(broadcast);

    let (closing_rx, closing_tx) = oneshot::channel();

    tokio::spawn(async move {
        let reason = loop {
            select! {
                res = commands.next() => {
                    match res {
                        Some(Command::Event(event)) => {
                            conn.write.send(event).await.expect("failed to send event");
                        }
                        Some(Command::Close) => {
                            break CloseReason::Request;
                        }
                        None => {
                            break CloseReason::Exhausted;
                        }
                    }
                }
                res = conn.read.next() => {
                    match res {
                        Some(Ok(event)) => {
                            event_sender.send((id, event.clone())).await.expect("server closed");

                            if let client::Event::Leave = event {
                                break CloseReason::Request;
                            }
                        }
                        Some(Err(e)) => {
                            error!("error in client ({id}): `{e}`");
                        }
                        // stream finished
                        None => {
                            break CloseReason::Exhausted;
                        }
                    }
                }
            }
        };

        let _ = closing_rx.send(reason);
    });

    closing_tx
}

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
