//! Implements voting and round handling.
use crate::author::Author;
use crate::error::Error;
use crate::event::RawEvent;
use crate::hash::Hash;
use crate::graph::Graph;
use serde::Serialize;

const FREQ_COIN_ROUNDS: usize = 10;

/// Rounds are a group events to be voted on.
#[derive(Clone, Debug)]
pub struct Round {
    /// Monotonically increasing round number.
    round: u64,
    /// Number of members in the population. Must be larger than one.
    authors: Box<[Author]>,
    /// Frequency of coin rounds. Must be larger than two.
    freq_coin_rounds: usize,
    /// Witnesses
    witnesses: Vec<Hash>,
}

impl Round {
    pub fn new(round: u64, authors: Box<[Author]>) -> Self {
        let witnesses = Vec::with_capacity(authors.len());
        Self {
            round,
            authors,
            witnesses,
            freq_coin_rounds: FREQ_COIN_ROUNDS,
        }
    }

    /// Round number.
    pub fn round(&self) -> u64 {
        self.round
    }

    /// Authors
    pub fn authors(&self) -> &[Author] {
        &self.authors
    }

    /// Population of a round.
    pub fn population(&self) -> usize {
        self.authors.len()
    }

    /// Supermajority threshold of a round.
    pub fn threshold(&self) -> usize {
        2 * self.population() / 3
    }

    /// Frequency of coin flipping rounds.
    pub fn freq_coin_rounds(&self) -> usize {
        self.freq_coin_rounds
    }

    /// Witnesses of a round.
    pub fn witnesses(&self) -> &[Hash] {
        &self.witnesses
    }
}

/// Voter splits events into rounds and orders them into a globally agreed
/// consensus order.
pub struct Voter<T> {
    graph: Graph<T>,
    rounds: Vec<Round>,
}

impl<T> Voter<T> {
    /// Creates a new voter.
    pub fn new() -> Self {
        Self {
            graph: Graph::new(),
            rounds: Vec::new(),
        }
    }

    /// Decide if a witness is famous.
    pub fn is_witness_famous(&self, _witness: &Hash, rounds: &[Round]) -> Option<bool> {
        let authors = rounds[0].authors();
        let threshold = rounds[0].threshold();
        let freq_coin_rounds = rounds[0].freq_coin_rounds() as usize;
        for diff in 1..rounds.len() {
            for wy in rounds[diff].witnesses() {
                let _strongly_seen_witnesses = rounds[diff - 1]
                    .witnesses()
                    .into_iter()
                    .filter(|w| self.graph.strongly_see(wy, w, authors));

                // TODO majority vote in strongly_seen_witnesses (is true for a tie)
                let vote = false;
                // TODO number of events in s with a vote of v
                let num_votes = 0;

                if diff == 1 { // first round of the election
                     // TODO y.vote <- can y see x
                } else {
                    if diff % freq_coin_rounds > 0 {
                        // this is a normal round
                        if num_votes > threshold {
                            // decide
                            // TODO wx.famous = vote
                            // TODO wy.vote = vote
                            return Some(vote);
                        }
                    } else {
                        // this is a coin round
                        if num_votes > threshold {
                            // vote
                            // wy.vote = vote
                        } else {
                            // flip a coin
                            // TODO wy.vote = f(wy.signature())
                        }
                    }
                }
            }
        }
        None
    }

    /// A round is famous when all it's witnesses are famous.
    pub fn famous_witnesses(&self, rounds: &[Round]) -> Option<Vec<Hash>> {
        let witnesses = rounds[0].witnesses();
        let mut famous_witnesses = Vec::with_capacity(witnesses.len());
        for witness in witnesses {
            if let Some(famous) = self.is_witness_famous(witness, rounds) {
                if famous {
                    famous_witnesses.push(*witness);
                }
            } else {
                return None;
            }
        }
        Some(famous_witnesses)
    }

    /// Iterates through rounds and performs a vote. If the fame of all witnesses
    /// is decided it calculates the order of events within a round and retires
    /// the round into history.
    pub fn process_rounds(&mut self) {
        /*for (i, round) in  self.rounds.iter().enumerate() {
            if let Some(_famous_witnesses) = self.famous_witnesses(&self.rounds[i..]) {
                // TODO order round and retire
                self.rounds = self.rounds[i..].to_vec();
            } else {
                break;
            }
        }*/
    }
}

impl<T: Serialize> Voter<T> {
    /// The maximum created round of all self parents of x (or 1 if there are none).
    /// Event x is a witness if x has a greater created round than its self parent.
    pub fn add_event<F: FnOnce() -> Result<Box<[Author]>, Error>>(
        &mut self,
        event: RawEvent<T>,
        start_round: F,
    ) -> Result<(), Error> {
        let hash = self.graph.add_event(event)?;
        let parent_round = self
            .graph
            .parents(self.graph.event(&hash).unwrap())
            .into_iter()
            .filter_map(|p| p.round_created())
            .max()
            .unwrap_or(1);
        let round = &self.rounds[parent_round as usize];
        let majority = round
            .witnesses()
            .into_iter()
            .filter(|w| {
                self.graph
                    .strongly_see(&hash, w, round.authors())
            })
            .nth(round.threshold() as usize)
            .is_some();
        let round = if majority {
            parent_round + 1
        } else {
            parent_round
        };
        let witness = round > parent_round;
        let mut event = self.graph.event_mut(&hash).unwrap();
        event.round_created = Some(round);
        event.witness = Some(witness);

        if self.rounds.last().map(|r| r.round < round).unwrap_or(true) {
            let authors = start_round()?;
            self.rounds.push(Round::new(round, authors));
        }
        self.rounds.last_mut().unwrap().witnesses.push(hash);
        self.process_rounds();
        Ok(())
    }
}
