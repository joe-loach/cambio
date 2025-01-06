use tokio::sync::mpsc;
use tracing::info;

use super::{config, player, GameData};

pub fn handler(
    data: GameData,
) -> (
    mpsc::Sender<(uuid::Uuid, player::CloseReason)>,
    mpsc::Receiver<uuid::Uuid>,
) {
    // listen for and remove shutdown clients
    let (shutdown, mut reasons) = mpsc::channel(config::MAX_PLAYER_COUNT);
    let (leaving, left) = mpsc::channel(1);
    tokio::spawn({
        async move {
            while let Some((id, _reason)) = reasons.recv().await {
                info!("client {id} left");
                data.lock().remove_player(id);
                let _ = leaving.send(id).await;
            }
        }
    });

    (shutdown, left)
}