use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use common::{
    event::server::{self, Winner},
    Card,
};
pub use game::Game;
use itertools::Itertools as _;
use tracing::debug;
use uuid::Uuid;

use crate::{Channels, GameData};

pub async fn run(game: &mut Game, data: &GameData, channels: &Channels) {
    let mut incoming = channels.incoming();
    let mut confirmed = HashSet::with_capacity(data.lock().player_count());

    'game_loop: loop {
        // make sure we process all events first
        if let Some(game_event) = game.poll_events() {
            match to_server_event_simple_broadcast(game_event) {
                Ok(event) => channels.broadcast_event(event).await,
                Err(complex_event) => match complex_event {
                    game::Event::Setup => {
                        setup(game, data);
                    }
                    game::Event::FirstPeek => first_peek(data, channels).await,
                    game::Event::StartTurn(turn) => start_turn(turn, data, channels).await,
                    game::Event::DrawCard(turn, card) => {
                        draw_card(card, turn, data, channels).await
                    }
                    game::Event::WaitForNewRound { confirmations } => {
                        ask_to_confirm(confirmations, &mut confirmed, channels).await
                    }
                    game::Event::FindWinner => find_winner(data, channels).await,
                    game::Event::Exit => break 'game_loop,
                    event => panic!("Unhandled game event: {event:?}"),
                },
            }

            continue;
        }

        debug!(state = ?game.current_state());

        if let Some(deadline) = game.poll_wait_deadline() {
            // wait to recieve something inside of the deadline
            if let Ok(Ok((id, event))) =
                tokio::time::timeout_at(deadline.into(), incoming.recv()).await
            {
                handle_incoming_event(game, data, event, id, &mut confirmed).await;
            }
        }

        // advance the game state
        game.advance();
    }

    // always make sure we tell the clients the game has ended
    channels.broadcast_event(server::Event::GameEnd).await;
}

async fn ask_to_confirm(confirmations: usize, confirmed: &mut HashSet<Uuid>, channels: &Channels) {
    // the first time this event is emitted (no confirms yet...)
    if confirmations == 0 {
        // erase previously confirmed people
        confirmed.clear();
        // only ask to confirm new round once
        channels
            .broadcast_event(server::Event::ConfirmNewRound)
            .await;
    }
}

fn get_id_from_turn(turn: usize, data: &GameData) -> Uuid {
    let data = data.lock();
    let index = turn % data.player_count();
    data.get_player(index).id()
}

fn setup(game: &mut Game, data: &GameData) {
    let mut data = data.lock();

    for i in 0..data.player_count() {
        common::data::take_starting_cards(&mut game.deck, &mut data, i);
    }

    debug!("set players up");
}

async fn first_peek(data: &GameData, channels: &Channels) {
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
            server::Event::FirstPeek(a, b)
        })
        .await;
}

async fn start_turn(turn: usize, data: &GameData, channels: &Channels) {
    let id = get_id_from_turn(turn, data);

    channels
        .broadcast_event(server::Event::TurnStart { id })
        .await;
}

async fn draw_card(card: Card, turn: usize, data: &GameData, channels: &Channels) {
    let id = get_id_from_turn(turn, data);

    channels.send(server::Event::DrawCard(card), id).await;
}

async fn find_winner(data: &GameData, channels: &Channels) {
    let winner_result = {
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

async fn handle_incoming_event(
    game: &mut Game,
    data: &GameData,
    event: common::event::client::Event,
    from_id: Uuid,
    confirmed: &mut HashSet<Uuid>,
) {
    use common::event::client::Event as ClientEvent;

    debug!(event = ?event, "handling event");

    match event {
        ClientEvent::Snap => game.handle_snap(Card::Joker, Instant::now()),
        ClientEvent::Decision(decision) => game.handle_decision(decision, Instant::now()),
        ClientEvent::ConfirmNewRound => {
            if confirmed.insert(from_id) {
                game.confirm_new_round(data.lock().player_count(), Instant::now())
            }
        }
        ClientEvent::SkipNewRound => game.skip_new_round(),
        _ => (),
    }
}

fn to_server_event_simple_broadcast(event: game::Event) -> Result<server::Event, game::Event> {
    let event = match event {
        game::Event::FirstDraw => server::Event::FirstDraw,
        game::Event::StartRound(round) => server::Event::RoundStart(round),
        game::Event::WaitForDecision => server::Event::WaitingForDecision,
        game::Event::WaitForSnap => server::Event::WaitingForSnap,
        game::Event::EndTurn(..) => server::Event::EndTurn,
        game::Event::EndRound(..) => server::Event::RoundEnd,
        game::Event::Cambio => server::Event::CambioCall,
        // ----
        game::Event::WaitForNewRound { .. } => return Err(event),
        game::Event::Setup => return Err(event),
        game::Event::DrawCard(..) => return Err(event),
        game::Event::Exit => return Err(event),
        game::Event::FirstPeek => return Err(event),
        game::Event::StartTurn(_) => return Err(event),
        game::Event::FindWinner => return Err(event),
    };

    Ok(event)
}
