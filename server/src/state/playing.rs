use std::collections::HashSet;

use common::{
    data::{self, Stage},
    event::{
        client,
        server::{self, Winner},
    },
    Deck,
};
use itertools::Itertools as _;
use tokio::{sync::mpsc, time};
use tracing::{info, trace};

use crate::{channels::ClientEvents, config::Config, Channels, Data, GameData, State};

pub async fn playing(mut data: Data) -> (State, Data) {
    let Data {
        ref config,
        data: ref mut game_data,
        ref channels,
        ref mut events,
        connect_enabled: _,
    } = data;

    super::notify_stage_change(Stage::Playing, channels, game_data);

    setup(channels, game_data).await;

    let mut rounds = 0;

    let state = loop {
        channels.lock().broadcast(server::Event::RoundStart(rounds));

        play_round(config, game_data, channels, events, rounds).await;

        channels.lock().broadcast(server::Event::RoundEnd);
        channels.lock().broadcast(server::Event::ConfirmNewRound);

        if !new_round(config, game_data, events).await {
            channels.lock().broadcast(server::Event::GameEnd);
            break State::Exit;
        }

        rounds += 1;
    };

    (state, data)
}

async fn play_round(
    config: &Config,
    data: &mut GameData,
    channels: &Channels,
    events: &mut mpsc::Receiver<ClientEvents>,
    round_offset: usize,
) {
    const FIRST_PLAYER: usize = 0;

    let player_count = data.lock().player_count();
    let mut turn = (FIRST_PLAYER + round_offset) % player_count;

    loop {
        channels.lock().broadcast(server::Event::TurnStart {
            uuid: data.lock().get_player(turn).id(),
        });

        let Some(card) = data.lock().deck.draw() else {
            channels.lock().broadcast(server::Event::EndTurn);

            break;
        };
        channels.lock().broadcast(server::Event::DrawCard(card));

        channels.lock().broadcast(server::Event::WaitingForDecision);

        // read decision
        while let Some((id, event)) = events.recv().await {
            if id == data.lock().get_player(turn).id() {
                if let client::Event::Decision = event {
                    break;
                }
            }
        }

        channels.lock().broadcast(server::Event::PlayAction);

        listen_for_snaps(config, channels, events).await;

        channels.lock().broadcast(server::Event::EndTurn);

        turn = (turn + 1) % player_count;
    }

    channels.lock().broadcast(server::Event::CambioCall);

    {
        let cooldown = time::Duration::from_secs(config.show_all_cooldown);
        time::sleep(cooldown).await;
    }

    let winner_result = {
        let conn = channels.lock();
        let data = data.lock();

        conn.broadcast(server::Event::ShowAll(data.players().to_vec()));

        let scores = data.players().iter().into_group_map_by(|p| p.score());
        let winner = scores
            .iter()
            .min_by(|(a, _), (b, _)| a.cmp(b))
            .and_then(|(_, players)| {
                if let [winner] = players.as_slice() {
                    Some(winner)
                } else {
                    // no sole winner
                    None
                }
            });

        if let Some(winner) = winner {
            Winner::Player { uuid: winner.id() }
        } else {
            Winner::Tied
        }
    };

    channels
        .lock()
        .broadcast(server::Event::Winner(winner_result));
}

async fn new_round(
    config: &Config,
    data: &GameData,
    events: &mut mpsc::Receiver<ClientEvents>,
) -> bool {
    let responses_needed = data.lock().player_count();

    let responses = {
        let timeout = time::sleep(time::Duration::from_secs(config.new_round_timer_secs));
        tokio::pin!(timeout);

        let mut responses = HashSet::new();

        loop {
            tokio::select! {
                Some((id, client::Event::Continue)) = events.recv() => {
                    responses.insert(id);

                    if responses.len() == responses_needed {
                        break;
                    }
                }
                _ = &mut timeout => { info!("new round time out"); break; }
                else => break,
            }
        }

        responses.len()
    };

    responses >= responses_needed
}

async fn listen_for_snaps(
    config: &Config,
    channels: &Channels,
    events: &mut mpsc::Receiver<ClientEvents>,
) -> Option<uuid::Uuid> {
    channels.lock().broadcast(server::Event::WaitingForSnap);

    let timeout = time::sleep(time::Duration::from_secs(config.snap_time_secs));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some((id, event)) = events.recv() => {
                if let client::Event::Snap = event {
                    info!("Player {id} snapped");
                    break Some(id);
                }
            }
            _ = &mut timeout => {
                info!("snap time out");
                break None;
            }
        }
    }
}

async fn setup(channels: &Channels, data: &mut GameData) {
    channels.lock().broadcast(server::Event::Setup);

    {
        let deck = &mut data.lock().deck;
        trace!("setting up deck");
        *deck = Deck::full();

        trace!("shuffling cards");
        let mut rng = rand::thread_rng();
        deck.shuffle(&mut rng);
    }

    {
        let mut data = data.lock();

        for i in 0..data.player_count() {
            data::take_starting_cards(&mut data, i);
        }
    }

    channels.lock().broadcast(server::Event::FirstDraw);

    let events = channels.lock().map_id(|id| {
        let data = data.lock();
        let p = data
            .players()
            .iter()
            .find(|p| p.id() == id)
            .expect("player no longer exists");
        let [a, b] = p.cards()[..2] else {
            unreachable!()
        };
        server::Event::FirstPeek(a, b)
    });
    events.await;
}
