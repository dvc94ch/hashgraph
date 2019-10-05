//! Implements voting and round handling.
use crate::event::{DerivedProperties, Event, RawEvent};
use crate::graph::Graph;

/// Rounds are a group events to be voted on.
#[derive(Clone, Debug)]
pub struct Round<'a> {
    /// Monotonically increasing round number.
    round: u32,
    /// Number of members in the population. Must be larger than one.
    population: u32,
    /// Frequency of coin rounds. Must be larger than two.
    freq_coin_rounds: u32,
    /// Witnesses
    witnesses: Vec<&'a Event>,
}

impl<'a> Round<'a> {
    /// Population of a round.
    pub fn population(&self) -> u32 {
        self.population
    }

    /// Supermajority threshold of a round.
    pub fn threshold(&self) -> u32 {
        2 * self.population / 3
    }

    /// Frequency of coin flipping rounds.
    pub fn freq_coin_rounds(&self) -> u32 {
        self.freq_coin_rounds
    }

    /// Witnesses of a round.
    pub fn witnesses(&self) -> &[&'a Event] {
        &self.witnesses
    }
}

/// Voter splits events into rounds and orders them into a globally agreed
/// consensus order.
pub struct Voter<'a> {
    graph: Graph<Event>,
    rounds: Vec<Round<'a>>,
    history: Vec<Round<'a>>,
}

impl<'a> Voter<'a> {
    /// Decide if a witness is famous.
    pub fn is_witness_famous(&self, _witness: &Event, rounds: &[Round]) -> Option<bool> {
        let n = rounds[0].population();
        let threshold = rounds[0].threshold();
        let freq_coin_rounds = rounds[0].freq_coin_rounds() as usize;
        for diff in 1..rounds.len() {
            for wy in rounds[diff].witnesses() {
                let _strongly_seen_witnesses = rounds[diff - 1]
                    .witnesses()
                    .into_iter()
                    .filter(|w| self.graph.strongly_see(*wy, w, n));

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
    pub fn famous_witnesses(&self, rounds: &[Round<'a>]) -> Option<Vec<&'a Event>> {
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
        let mut i = 0;
        while i < self.rounds.len() - 1 {
            if let Some(_famous_witnesses) = self.famous_witnesses(&self.rounds[i..]) {
                // TODO order round and retire
                self.history.push(self.rounds[i].clone());
            } else {
                break;
            }
            i += 1;
        }
        if i > 0 {
            self.rounds = self.rounds[i..].to_vec();
        }
    }

    /// The maximum created round of all self parents of x (or 1 if there are none).
    /// Event x is a witness if x has a greater created round than its self parent.
    pub fn add_event(&mut self, raw: RawEvent) {
        let parent_round = self
            .graph
            .parents(&raw)
            .into_iter()
            .map(|p| p.round())
            .max()
            .unwrap_or(1);
        let round = &self.rounds[parent_round as usize];
        let majority = round
            .witnesses()
            .into_iter()
            .filter(|w| {
                self.graph
                    .strongly_see(&raw, (**w).into(), round.threshold())
            })
            .nth(round.threshold() as usize);
        let round = if majority.is_some() {
            parent_round + 1
        } else {
            parent_round
        };
        let witness = round > parent_round;
        let event = Event {
            raw,
            round,
            witness,
        };
        self.graph.add_event(event);
        self.process_rounds();
    }
}
