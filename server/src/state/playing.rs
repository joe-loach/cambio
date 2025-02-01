use std::collections::{HashMap, HashSet};

use common::{
    data::{self, Stage},
    event::{
        client,
        server::{self, Winner},
    },
    Deck,
};
use itertools::Itertools as _;
use tokio::time;
use tracing::{info, trace};

use crate::{config::Config, player, Channels, Data, GameData, State};

pub async fn playing(mut data: Data) -> (State, Data) {
    let Data {
        ref config,
        data: ref mut game_data,
        ref channels,
        connect_enabled: _,
    } = data;

    super::notify_stage_change(Stage::Playing, channels, game_data).await;

    setup(channels, game_data).await;

    let mut rounds = 0;

    let state = loop {
        channels
            .broadcast_event(server::Event::RoundStart(rounds))
            .await;

        play_round(config, game_data, channels, rounds).await;

        channels.broadcast_event(server::Event::RoundEnd).await;
        channels
            .broadcast_event(server::Event::ConfirmNewRound)
            .await;

        if !new_round(config, game_data, channels).await {
            channels.broadcast_event(server::Event::GameEnd).await;
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
    round_offset: usize,
) {
    const FIRST_PLAYER: usize = 0;

    let player_count = data.lock().player_count();
    let mut turn = (FIRST_PLAYER + round_offset) % player_count;

    loop {
        let turn_id = data.lock().get_player(turn).id();
        channels
            .broadcast_event(server::Event::TurnStart { uuid: turn_id })
            .await;

        let Some(card) = data.lock().deck.draw() else {
            channels.broadcast_event(server::Event::EndTurn).await;

            break;
        };
        channels
            .broadcast_event(server::Event::DrawCard(card))
            .await;

        channels
            .broadcast_event(server::Event::WaitingForDecision)
            .await;

        // read decision
        let mut incoming = channels.incoming();
        let decision = loop {
            if let Ok((id, event)) = incoming.recv().await {
                if id == data.lock().get_player(turn).id() {
                    if let client::Event::Decision(decision) = event {
                        break decision;
                    }
                }
            } else {
                panic!("error receiving decision");
            }
        };
        info!("client {turn_id} chose {decision:?}");

        channels.broadcast_event(server::Event::PlayAction).await;

        listen_for_snaps(config, channels).await;

        channels.broadcast_event(server::Event::EndTurn).await;

        turn = (turn + 1) % player_count;
    }

    channels.broadcast_event(server::Event::CambioCall).await;

    {
        let cooldown = time::Duration::from_secs(config.show_all_cooldown);
        time::sleep(cooldown).await;
    }

    let winner_result = {
        let players = { data.lock().players().to_vec() };

        channels
            .broadcast_event(server::Event::ShowAll(players))
            .await;

        let data = data.lock();
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
        .broadcast_event(server::Event::Winner(winner_result))
        .await;
}

async fn new_round(config: &Config, data: &GameData, channels: &Channels) -> bool {
    let responses_needed = data.lock().player_count();

    let responses = {
        let timeout = time::sleep(time::Duration::from_secs(config.new_round_timer_secs));
        tokio::pin!(timeout);

        let mut responses = HashSet::new();

        let mut incoming = channels.incoming();

        loop {
            tokio::select! {
                Ok((id, client::Event::Continue)) = incoming.recv() => {
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

async fn listen_for_snaps(config: &Config, channels: &Channels) -> Option<uuid::Uuid> {
    channels
        .broadcast_event(server::Event::WaitingForSnap)
        .await;

    let timeout = time::sleep(time::Duration::from_secs(config.snap_time_secs));
    tokio::pin!(timeout);

    let mut incoming = channels.incoming();
    loop {
        tokio::select! {
            Ok((id, event)) = incoming.recv() => {
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
    channels.broadcast_event(server::Event::Setup).await;

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

    channels.broadcast_event(server::Event::FirstDraw).await;

    let first_cards = data
        .lock()
        .players()
        .iter()
        .map(|p| {
            let id = p.id();
            let [a, b] = p.cards()[..2] else {
                unreachable!()
            };

            (id, (a, b))
        })
        .collect::<HashMap<_, _>>();

    channels
        .broadcast_map(move |id| {
            let (a, b) = first_cards[&id];
            player::Command::Event(server::Event::FirstPeek(a, b))
        })
        .await;
}
