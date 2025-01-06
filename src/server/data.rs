use crate::{Deck, STARTING_DECK_LEN};

use super::player::PlayerData;

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

pub fn take_starting_cards(data: &mut GameData, player: usize) {
    let cards_from_deck = data.deck.0.drain(..STARTING_DECK_LEN);
    data.players[player].cards.extend(cards_from_deck);
}
