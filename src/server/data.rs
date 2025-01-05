use crate::{player::PlayerData, Deck};

pub struct GameData {
    pub deck: Deck,
    pub(super) players: Vec<PlayerData>,
}

impl GameData {
    pub fn initial() -> Self {
        GameData {
            deck: Deck::full(),
            players: Vec::new(),
        }
    }

    pub fn add_player(&mut self, player: PlayerData) -> usize {
        let index = self.players.len();
        self.players.push(player);
        index
    }

    pub fn player(&self, i: usize) -> &PlayerData {
        &self.players[i]
    }

    pub fn player_mut(&mut self, i: usize) -> &mut PlayerData {
        &mut self.players[i]
    }
}