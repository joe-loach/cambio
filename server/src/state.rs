pub mod lobby;
pub mod playing;

use common::{data::Stage, event::server};
use futures::{future::BoxFuture, FutureExt};
use tokio::sync::{mpsc, oneshot};

use crate::{Channels, GameData};

pub fn notify_stage_change(stage: Stage, channels: &Channels, data: &mut GameData) {
    data.lock().stage = stage;
    channels.lock().broadcast(server::Event::StageChange(stage));
}

pub fn host_id(data: &GameData) -> Option<uuid::Uuid> {
    data.lock().players().first().map(|p| p.id())
}

pub type Sender<R> = mpsc::Sender<(BoxFuture<'static, R>, oneshot::Sender<R>)>;

pub async fn process<R>(tx: &Sender<R>, fut: BoxFuture<'static, R>) -> R {
    let (resp_tx, resp_rx) = oneshot::channel();
    let _ = tx.send((fut, resp_tx)).await;
    resp_rx.await.expect("Runner closed")
}

/// Interruptable Task Runner
///
/// Runs futures received by a channel, in a loop.
/// If the future finishes, the value is returned to the caller.
pub fn runner<R>() -> Sender<R>
where
    R: Send + 'static,
{
    let (tx, mut rx) = mpsc::channel(128);

    tokio::spawn(async move {
        let mut task = std::future::pending().boxed();
        let mut response: Option<oneshot::Sender<R>> = None;

        // used to make sure we don't poll completed futures
        let mut finished = true;

        loop {
            tokio::select! {
                biased;
                res = rx.recv() => {
                    match res {
                        // new future to run, replace old one
                        Some((fut, resp)) => {
                            response = Some(resp);
                            task = fut;
                            finished = false;
                        }
                        None => break,
                    }
                }
                res = &mut task, if !finished => {
                    finished = true;
                    if let Some(resp) = response.take() {
                        let _ = resp.send(res);
                    }
                }
            }
        }
    });

    tx
}
