use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Card;

use crate::data::PlayerData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    /// Ready to start serving event loop for client.
    Enter,
    /// The game has restarted
    Restart,
    /// Information about the lobby.
    /// 
    /// Can be requested any time.
    LobbyInfo {
        player_count: usize,
    },
    /// Response to client `Join` request.
    /// 
    /// Must not be broadcasted.
    AssignId {
        id: Uuid,
    },
    /// A player joined
    Joined {
        id: Uuid,
    },
    /// A player left
    Left {
        id: Uuid,
    },
    /// Start of a round
    RoundStart(usize),
    /// Reset cards and shuffle
    Setup,
    /// Players draw their first 4 cards
    ///
    /// Clients do not need to know about their cards.
    FirstDraw,
    /// Players view their front 2 cards
    FirstPeek(Card, Card),
    /// Turn of player has started
    TurnStart { id: Uuid },
    /// Card is drawn from deck
    DrawCard(Card),
    /// Waiting for the player to make a decision
    WaitingForDecision,
    /// Play the action of the card
    PlayAction,
    /// Wait for a potential snap
    WaitingForSnap,
    /// Turn has ended
    EndTurn,
    /// Cambio has been called
    CambioCall,
    /// Show all cards
    ShowAll(Vec<PlayerData>),
    /// Announce winner
    Winner(Winner),
    /// End of round
    RoundEnd,
    /// Ask all clients to config if they wish to play again.
    ConfirmNewRound,
    /// Game ended
    GameEnd,
    /// Server Closing
    ServerClosing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Winner {
    Player { uuid: Uuid },
    Tied,
}
