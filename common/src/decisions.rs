use serde::{Deserialize, Serialize};

use crate::{Card, Face};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Decision {
    /// Place the card into the discard pile.
    Discard,
    /// Replace the card in hand with one in the deck, discarding it.
    Replace,
    /// Discard the card and look at one of your own cards.
    LookAtOwn,
    /// Discard the card and any card other than your own.
    LookAtOther,
    /// Choose a card of your own and someone elses to swap, without looking.
    BlindSwap,
    /// Look at one of your own cards, and one of someone elses, then choose whether to swap them.
    LookAndSwap,
}

impl Decision {
    /// All of the decisions from [`Decision`].
    pub const ALL: [Decision; 6] = [
        Decision::Discard,
        Decision::Replace,
        Decision::LookAtOwn,
        Decision::LookAtOther,
        Decision::BlindSwap,
        Decision::LookAndSwap,
    ];

    /// Checks that this decision is part of a valid set.
    pub fn is_valid(&self, valid_decisions: DecisionSet) -> bool {
        valid_decisions.contains(self)
    }

    const fn discriminant(&self) -> u8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        unsafe { *(self as *const Self as *const u8) }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DecisionSet(u64);

/// Returns the set of valid decisions given the card.
pub const fn valid_set(card: Card) -> DecisionSet {
    // All cards can Discard and Replace.
    const BASE: DecisionSet = DecisionSet::from_array([Decision::Discard, Decision::Replace]);

    match card {
        Card::Normal { face, .. } => match face {
            Face::King => BASE.and(Decision::LookAndSwap),
            Face::Jack | Face::Queen => BASE.and(Decision::BlindSwap),
            Face::Nine | Face::Ten => BASE.and(Decision::LookAtOther),
            Face::Seven | Face::Eight => BASE.and(Decision::LookAtOwn),
            _ => BASE,
        },
        Card::Joker => BASE,
    }
}

impl DecisionSet {
    pub const EMPTY: DecisionSet = DecisionSet(0_u64);

    pub const fn new() -> Self {
        Self::EMPTY
    }

    pub const fn only(decision: Decision) -> Self {
        DecisionSet(Self::to_bit(decision))
    }

    pub fn into_vec(self) -> Vec<Decision> {
        Decision::ALL.into_iter().filter(|decision| self.contains(decision)).collect()
    }

    pub const fn from_array<const N: usize>(decisions: [Decision; N]) -> Self {
        let mut this = Self::new();
        let mut i = 0;
        while i < N {
            this = this.and(decisions[i]);
            i += 1;
        }
        this
    }

    pub fn contains(&self, decision: &Decision) -> bool {
        let bit = Self::to_bit(*decision);
        bit == self.0 & bit
    }
    pub const fn and(mut self, decision: Decision) -> Self {
        self.0 |= Self::to_bit(decision);
        self
    }

    #[inline(always)]
    const fn to_bit(decision: Decision) -> u64 {
        1_u64 << decision.discriminant()
    }
}

impl Default for DecisionSet {
    fn default() -> Self {
        Self::new()
    }
}

#[test]
fn set_impl() {
    let set = const { DecisionSet::from_array([Decision::Discard, Decision::Replace]) };
    assert!(set.contains(&Decision::Discard));
    assert!(set.contains(&Decision::Replace));

    let set = const { DecisionSet::only(Decision::Discard) };

    assert!(set.contains(&Decision::Discard));
    assert!(!set.contains(&Decision::Replace));
}
