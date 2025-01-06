use serde::{Deserialize, Serialize};

use crate::{Card, Deck, STARTING_DECK_LEN};

pub struct GameData {
    pub deck: Deck,
    players: Vec<PlayerData>,
}

impl GameData {
    pub fn new() -> Self {
        GameData {
            deck: Deck::full(),
            players: Vec::new(),
        }
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn add_player(&mut self, player: PlayerData) -> usize {
        let index = self.players.len();
        self.players.push(player);
        index
    }

    pub fn remove_player(&mut self, id: uuid::Uuid) {
        if let Some(index) = self.players.iter().position(|p| p.id() == id) {
            self.players.remove(index);
        }
    }

    pub fn players(&self) -> &[PlayerData] {
        &self.players
    }

    pub fn get_player(&self, i: usize) -> &PlayerData {
        &self.players[i]
    }
}

impl Default for GameData {
    fn default() -> Self {
        Self::new()
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

pub fn take_starting_cards(data: &mut GameData, player: usize) {
    let cards_from_deck = data.deck.0.drain(..STARTING_DECK_LEN);
    data.players[player].cards.extend(cards_from_deck);
}
