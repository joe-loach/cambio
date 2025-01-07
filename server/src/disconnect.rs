use common::data::Stage;
use tokio::sync::mpsc;
use tracing::info;

use super::{config, player, GameData, Interrupt};

pub fn handler(
    data: GameData,
    ir_tx: mpsc::Sender<Interrupt>,
) -> (
    mpsc::Sender<(uuid::Uuid, player::CloseReason)>,
    mpsc::Receiver<uuid::Uuid>,
    tokio::task::AbortHandle,
) {
    // listen for and remove shutdown clients
    let (shutdown, mut reasons) = mpsc::channel(config::MAX_PLAYER_COUNT);
    let (leaving, left) = mpsc::channel(1);
    let join = tokio::spawn({
        async move {
            while let Some((id, _reason)) = reasons.recv().await {
                info!("client {id} left");
                data.lock().remove_player(id);
                let _ = leaving.send(id).await;

                let stage = data.lock().stage;
                match stage {
                    Stage::Lobby => (),
                    Stage::Playing => {
                        // reset the game back when there aren't enough players to play
                        if data.lock().player_count() < config::MIN_PLAYER_COUNT {
                            let _ = ir_tx.send(Interrupt::Restart).await;
                        }
                    },
                }
            }
        }
    });
    let abort = join.abort_handle();

    (shutdown, left, abort)
}
