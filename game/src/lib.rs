use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use common::{decisions::Decision, Card, Deck};

#[derive(Debug, Clone, Copy)]
pub enum State {
    Pregame,
    StartRound {
        round: usize,
        reset_deck: bool,
    },
    StartTurn {
        round: usize,
        turn: usize,
    },
    DrawCard {
        round: usize,
        turn: usize,
        card: Card,
    },
    WaitingForDecision {
        round: usize,
        turn: usize,
        started: Instant,
    },
    PlayDecision {
        round: usize,
        turn: usize,
        decision: Decision,
    },
    WaitingForSnaps {
        round: usize,
        turn: usize,
        started: Instant,
    },
    Snapped {
        round: usize,
        turn: usize,
        card: Card,
    },
    EndTurn {
        round: usize,
        turn: usize,
    },
    EndRound {
        round: usize,
    },
    FindWinner {
        round: usize,
    },
    WaitingForNewRound {
        round: usize,
        confirmations: usize,
        started: Instant,
    },
    CambioCall {
        round: usize,
    },
    Finished,
}

#[derive(Debug)]
pub enum Event {
    Setup,
    FirstDraw,
    FirstPeek,
    StartRound(usize),
    StartTurn(usize),
    DrawCard(usize, Card),
    WaitForDecision,
    WaitForSnap,
    EndTurn(usize),
    WaitForNewRound { confirmations: usize },
    EndRound(usize),
    Cambio,
    FindWinner,
    Exit,
}

pub struct Game {
    pub deck: Deck,
    state: State,
    events: VecDeque<Event>,
}

impl Game {
    /// Advance the state of the game.
    pub fn advance(&mut self) {
        self.state = match self.state {
            State::Pregame => State::StartRound {
                round: 0,
                reset_deck: false,
            },
            State::StartRound { round, reset_deck } => {
                self.output_event(Event::Setup);
                // reset and shuffle the cards each round
                if reset_deck {
                    self.deck = Deck::full();
                }
                let mut rng = rand::thread_rng();
                self.deck.shuffle(&mut rng);
                // each round should start with a different player than the last
                // offset the turn by the number of rounds
                self.output_event(Event::StartRound(round));
                let turn_offset = round;
                State::StartTurn {
                    round,
                    turn: turn_offset,
                }
            }
            State::StartTurn { round, turn } => {
                self.output_event(Event::StartTurn(turn));

                if let Some(card) = self.deck.draw() {
                    State::DrawCard { round, turn, card }
                } else {
                    // no more cards in deck
                    State::EndRound { round }
                }
            }
            State::DrawCard { round, turn, card } => {
                self.output_event(Event::DrawCard(turn, card));

                self.output_event(Event::WaitForDecision);
                State::WaitingForDecision {
                    round,
                    turn,
                    started: Instant::now(),
                }
            }
            State::WaitingForDecision {
                round,
                turn,
                started,
            } => {
                if started.elapsed() >= Self::MAX_DECISION_TIME {
                    // waited too long to decide
                    // player does nothing,
                    // end turn (no need to wait for snaps)
                    State::EndTurn { round, turn }
                } else {
                    // keep waiting
                    self.output_event(Event::WaitForDecision);
                    State::WaitingForDecision {
                        round,
                        turn,
                        started,
                    }
                }
            }
            State::PlayDecision {
                round,
                turn,
                decision,
            } => {
                // TODO: actually play the decision
                let _ = decision;

                self.output_event(Event::WaitForSnap);
                State::WaitingForSnaps {
                    round,
                    turn,
                    started: Instant::now(),
                }
            }
            State::WaitingForSnaps {
                round,
                turn,
                started,
            } => {
                if started.elapsed() >= Self::MAX_SNAP_TIME {
                    // waited too long for a snap
                    // let's end the turn
                    State::EndTurn { round, turn }
                } else {
                    // keep waiting
                    self.output_event(Event::WaitForSnap);
                    State::WaitingForSnaps {
                        round,
                        turn,
                        started,
                    }
                }
            }
            State::Snapped { round, turn, card } => {
                // TODO: actually snap the card
                let _ = card;

                State::EndTurn { round, turn }
            }
            State::EndTurn { round, turn } => {
                self.output_event(Event::EndTurn(turn));

                // keep playing this round whilst there are still cards left
                if self.deck.is_empty() {
                    State::EndRound { round }
                } else {
                    let next_turn = turn + 1;
                    State::StartTurn {
                        round,
                        turn: next_turn,
                    }
                }
            }
            State::CambioCall { round } => {
                self.output_event(Event::Cambio);
                State::EndRound { round }
            }
            State::EndRound { round } => {
                self.output_event(Event::EndRound(round));
                State::FindWinner { round }
            }
            State::FindWinner { round } => {
                self.output_event(Event::FindWinner);

                // start at no confirmations
                let confirmations = 0;
                self.output_event(Event::WaitForNewRound { confirmations });
                State::WaitingForNewRound {
                    round,
                    confirmations,
                    started: Instant::now(),
                }
            }
            State::WaitingForNewRound {
                round,
                confirmations,
                started,
            } => {
                if started.elapsed() >= Self::MAX_NEW_ROUND_CONFIRM_TIME {
                    // waited too long for a new round
                    // let's end the game
                    State::Finished
                } else {
                    // keep waiting otherwise...
                    self.output_event(Event::WaitForNewRound { confirmations });
                    State::WaitingForNewRound {
                        round,
                        confirmations,
                        started,
                    }
                }
            }
            State::Finished => {
                self.output_event(Event::Exit);
                // remain in the Finished state
                State::Finished
            }
        };
    }
}

impl Game {
    /// Create a new [`Game`] in the [`State::Pregame`] state.
    pub fn new() -> Self {
        Self::new_with(Deck::full(), State::Pregame)
    }

    /// Create a new [`Game`] from a [`Deck`] and [`State`].
    pub fn new_with(deck: Deck, state: State) -> Self {
        Self {
            deck,
            state,
            events: VecDeque::new(),
        }
    }

    /// Pop off an [`Event`], if there is any.
    pub fn poll_events(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Call Cambio!
    ///
    /// Returns `true` if the call was successful.
    /// If `false`, the game was in an invalid state to call cambio
    pub fn cambio_call(&mut self) -> bool {
        match self.state {
            State::WaitingForDecision { round, .. } | State::WaitingForSnaps { round, .. } => {
                self.state = State::CambioCall { round }
            }
            // PlayDecision changes deck state
            // so we have to advance first before calling cambio
            State::PlayDecision { round, .. } => {
                // only needs one advance, it always resolves
                self.advance();
                self.state = State::CambioCall { round };
            }
            _ => return false,
        }
        true
    }

    const MAX_DECISION_TIME: Duration = Duration::from_secs(10);

    /// A decision has been made whilst waiting.
    ///
    /// Because we match on None, only the **first** decision is remembered.
    pub fn handle_decision(&mut self, decision: Decision, decided_at: Instant) {
        if let State::WaitingForDecision {
            round,
            turn,
            started,
        } = self.state
        {
            let elapsed = decided_at.duration_since(started);

            if elapsed <= Self::MAX_DECISION_TIME {
                self.state = State::PlayDecision {
                    round,
                    turn,
                    decision,
                }
            } else {
                // waited too long to decide
                // player does nothing,
                // end turn (no need to wait for snaps)
                self.state = State::EndTurn { round, turn };
            }
        }
    }

    const MAX_SNAP_TIME: Duration = Duration::from_secs(2);

    /// Snap!
    ///
    /// We might've received it too late however,
    /// so we check if it's a valid snap first.
    pub fn handle_snap(&mut self, card: Card, snapped_at: Instant) {
        if let State::WaitingForSnaps {
            round,
            turn,
            started,
        } = self.state
        {
            let elapsed = snapped_at.duration_since(started);

            if elapsed <= Self::MAX_SNAP_TIME {
                // snap!
                self.state = State::Snapped { round, turn, card };
            } else {
                // waited too long for a snap
                // let's end the turn
                self.state = State::EndTurn { round, turn };
            }
        }
    }

    const MAX_NEW_ROUND_CONFIRM_TIME: Duration = Duration::from_secs(10);

    /// Confirm a new round if we're waiting.
    pub fn confirm_new_round(&mut self, needed_confirms: usize, confirmed_at: Instant) {
        if let State::WaitingForNewRound {
            started,
            round,
            confirmations,
        } = &mut self.state
        {
            let elapsed = confirmed_at.duration_since(*started);

            // increase confirmations and check if there are enough to move on
            *confirmations += 1;

            if elapsed <= Self::MAX_NEW_ROUND_CONFIRM_TIME {
                if *confirmations >= needed_confirms {
                    // start a new round
                    let next_round = *round + 1;
                    self.state = State::StartRound {
                        round: next_round,
                        // make sure we reset the deck
                        reset_deck: true,
                    };
                }
            } else {
                // waited too long for a new round
                // let's end the game
                self.state = State::Finished;
            }
        }
    }

    /// Skip playing another round.
    pub fn skip_new_round(&mut self) {
        if let State::WaitingForNewRound { .. } = self.state {
            self.state = State::Finished;
        }
    }

    /// Get the deadline for a waiting period.
    pub fn poll_wait_deadline(&self) -> Option<Instant> {
        match self.state {
            State::WaitingForDecision { started, .. } => Some(started + Self::MAX_DECISION_TIME),
            State::WaitingForSnaps { started, .. } => Some(started + Self::MAX_SNAP_TIME),
            State::WaitingForNewRound { started, .. } => {
                Some(started + Self::MAX_NEW_ROUND_CONFIRM_TIME)
            }
            _ => None,
        }
    }

    pub fn current_state(&self) -> &State {
        &self.state
    }

    fn output_event(&mut self, event: Event) {
        self.events.push_back(event);
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn one_round_finish() {
    let mut game = Game::new();

    const MAX_ITERS: usize = 1000;
    for _ in 0..MAX_ITERS {
        if let Some(event) = game.poll_events() {
            println!("{:?}", event);

            match event {
                Event::WaitForDecision => game.handle_decision(Decision::Replace, Instant::now()),
                Event::WaitForSnap => game.handle_snap(Card::Joker, Instant::now()),
                Event::WaitForNewRound { .. } => game.skip_new_round(),
                Event::Exit => break,
                _ => (),
            }

            // poll all events first
            continue;
        }

        // if there is a deadline, wait for it
        if let Some(deadline) = game.poll_wait_deadline() {
            let now = Instant::now();

            if let Some(delay) = deadline.checked_duration_since(now) {
                std::thread::sleep(delay);
            }
        }

        // advance the game state
        game.advance();
    }

    assert!(matches!(game.state, State::Finished));
}
