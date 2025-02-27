pub mod data;
pub mod event;
pub mod stream;
pub mod decisions;

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

macro_rules! SuitSet {
    ($name:ident) => {
        struct $name;
        impl $name {
            pub const fn number(self, n: u8) -> Card {
                Card::Normal {
                    suit: Suit::$name,
                    face: Face::from_number(n),
                }
            }

            pub const fn king(self) -> Card {
                Card::Normal {
                    suit: Suit::$name,
                    face: Face::King,
                }
            }

            pub const fn queen(self) -> Card {
                Card::Normal {
                    suit: Suit::$name,
                    face: Face::Queen,
                }
            }

            pub const fn jack(self) -> Card {
                Card::Normal {
                    suit: Suit::$name,
                    face: Face::Jack,
                }
            }
        }
    };
}

SuitSet!(Hearts);
SuitSet!(Diamonds);
SuitSet!(Clubs);
SuitSet!(Spades);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Card {
    Normal { suit: Suit, face: Face },
    Joker,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

impl Suit {
    pub const fn color(&self) -> Color {
        match self {
            Suit::Hearts => Color::Red,
            Suit::Diamonds => Color::Red,
            Suit::Clubs => Color::Black,
            Suit::Spades => Color::Black,
        }
    }
}

impl Card {
    pub const fn game_value(&self) -> i8 {
        match self {
            Card::Normal { suit, face } => {
                let color = suit.color();
                match (color, face) {
                    (Color::Red, Face::King) => -2,
                    (Color::Black, Face::King) => 13,
                    (_, Face::Queen) => 12,
                    (_, Face::Jack) => 11,
                    (_, Face::Ten) => 10,
                    (_, Face::Nine) => 9,
                    (_, Face::Eight) => 8,
                    (_, Face::Seven) => 7,
                    (_, Face::Six) => 6,
                    (_, Face::Five) => 5,
                    (_, Face::Four) => 4,
                    (_, Face::Three) => 3,
                    (_, Face::Two) => 2,
                    (_, Face::Ace) => 1,
                }
            }
            Card::Joker => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Face {
    King,
    Queen,
    Jack,
    Ten,
    Nine,
    Eight,
    Seven,
    Six,
    Five,
    Four,
    Three,
    Two,
    Ace,
}

impl Face {
    pub const fn from_number(n: u8) -> Self {
        match n {
            10 => Face::Ten,
            9 => Face::Nine,
            8 => Face::Eight,
            7 => Face::Seven,
            6 => Face::Six,
            5 => Face::Five,
            4 => Face::Four,
            3 => Face::Three,
            2 => Face::Two,
            1 => Face::Ace,
            _ => panic!("Not a face"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Black,
}

pub const FULL_DECK: [Card; 54] = [
    Hearts.king(),
    Hearts.queen(),
    Hearts.jack(),
    Hearts.number(10),
    Hearts.number(9),
    Hearts.number(8),
    Hearts.number(7),
    Hearts.number(6),
    Hearts.number(5),
    Hearts.number(4),
    Hearts.number(3),
    Hearts.number(2),
    Hearts.number(1),
    Diamonds.king(),
    Diamonds.queen(),
    Diamonds.jack(),
    Diamonds.number(10),
    Diamonds.number(9),
    Diamonds.number(8),
    Diamonds.number(7),
    Diamonds.number(6),
    Diamonds.number(5),
    Diamonds.number(4),
    Diamonds.number(3),
    Diamonds.number(2),
    Diamonds.number(1),
    Clubs.king(),
    Clubs.queen(),
    Clubs.jack(),
    Clubs.number(10),
    Clubs.number(9),
    Clubs.number(8),
    Clubs.number(7),
    Clubs.number(6),
    Clubs.number(5),
    Clubs.number(4),
    Clubs.number(3),
    Clubs.number(2),
    Clubs.number(1),
    Spades.king(),
    Spades.queen(),
    Spades.jack(),
    Spades.number(10),
    Spades.number(9),
    Spades.number(8),
    Spades.number(7),
    Spades.number(6),
    Spades.number(5),
    Spades.number(4),
    Spades.number(3),
    Spades.number(2),
    Spades.number(1),
    Card::Joker,
    Card::Joker,
];

pub const STARTING_DECK_LEN: usize = 4;

#[derive(Serialize, Deserialize)]
pub struct Deck(Vec<Card>);

impl Deck {
    pub fn full() -> Self {
        Deck(FULL_DECK.to_vec())
    }

    pub fn draw(&mut self) -> Option<Card> {
        self.0.pop()
    }

    pub fn shuffle<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) {
        self.0.shuffle(rng);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
