use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;

use crate::{client, server, stream, Card, Deck, STARTING_DECK_LEN};

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerData {
    id: uuid::Uuid,
    pub(crate) cards: Vec<Card>,
}

impl PartialEq for PlayerData {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for PlayerData {}

impl PlayerData {
    pub fn new() -> Self {
        let id = uuid::Uuid::new_v4();
        Self {
            id,
            cards: Vec::with_capacity(STARTING_DECK_LEN),
        }
    }

    pub fn id(&self) -> uuid::Uuid {
        self.id
    }

    pub fn score(&self) -> i32 {
        self.cards.iter().map(|c| c.game_value() as i32).sum()
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn cards_mut(&mut self) -> &mut [Card] {
        &mut self.cards
    }
}

impl Default for PlayerData {
    fn default() -> Self {
        Self::new()
    }
}

pub fn take_starting_cards(player: &mut PlayerData, deck: &mut Deck) {
    let cards_from_deck = deck.0.drain(..STARTING_DECK_LEN);
    player.cards.extend(cards_from_deck);
}