use futures::SinkExt;
use tokio::sync::mpsc;
use tokio::{net::TcpStream, select};
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::{client, server, stream};

use super::Connections;

#[derive(Debug)]
pub enum CloseReason {
    Server,
    Client,
    Error,
}

#[derive(Debug, Clone)]
pub enum Command {
    Event(server::Event),
    Close,
}

pub fn spawn(
    connections: &mut Connections,
    id: uuid::Uuid,
    mut conn: PlayerConn,
    shutdown: mpsc::Sender<(uuid::Uuid, CloseReason)>,
) {
    const COMMAND_CHANNEL_CAPACITY: usize = 32;

    let event_sender = connections.event_sender();

    let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
    connections.insert(id, tx);

    // turn channels into streams
    let own = Box::pin(ReceiverStream::new(rx));
    let broadcast =
        Box::pin(BroadcastStream::new(connections.subscribe_to_all()).filter_map(|res| res.ok()));

    // combine commands from both sources
    let mut commands = own.merge(broadcast);

    tokio::spawn(async move {
        let reason = loop {
            select! {
                Some(cmd) = commands.next() => {
                    match cmd {
                        Command::Event(event) => {
                            conn.write.send(event).await.expect("failed to send event");
                        }
                        Command::Close => {
                            break CloseReason::Server;
                        }
                    }
                }
                Some(Ok(event)) = conn.read.next() => {
                    event_sender.send((id, event.clone())).await.expect("server closed");

                    if let client::Event::Leave = event {
                        break CloseReason::Client;
                    }
                }
                else => {
                    break CloseReason::Error;
                }
            }
        };

        let _ = shutdown.send((id, reason)).await;
    });
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
