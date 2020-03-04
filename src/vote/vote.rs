//! Implements voting and round handling.
use crate::author::Author;
use crate::error::Error;
use crate::event::RawEvent;
use crate::graph::Graph;
use crate::hash::Hash;
use serde::Serialize;

const FREQ_COIN_ROUNDS: usize = 10;

/// Rounds are a group events to be voted on.
#[derive(Clone, Debug)]
pub struct Round {
    /// Monotonically increasing round number.
    round: u64,
    /// Block number.
    block: u64,
    /// Number of members in the population. Must be larger than one.
    authors: Box<[Author]>,
    /// Frequency of coin rounds. Must be larger than two.
    freq_coin_rounds: usize,
    /// Witnesses
    witnesses: Vec<Hash>,
}

impl Round {
    pub fn new(round: u64, block: u64, authors: Box<[Author]>) -> Self {
        let witnesses = Vec::with_capacity(authors.len());
        Self {
            round,
            block,
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

impl<T: Serialize> Voter<T> {
    pub fn new() -> Self {
        Self {
            graph: Graph::default(),
            rounds: Default::default(),
        }
    }

    pub fn graph(&self) -> &Graph<T> {
        &self.graph
    }

    pub fn rounds(&self) -> &[Round] {
        &self.rounds
    }

    pub fn sync_state(&self) -> (u64, Box<[Option<u64>]>) {
        let round = self.rounds.last().unwrap();
        (round.block, self.graph.sync_state(&round.authors))
    }

    pub fn sync(
        &self,
        state: (u64, Box<[Option<u64>]>),
    ) -> Result<impl Iterator<Item = &RawEvent<T>>, Error> {
        let (block, seq) = state;
        let authors = self
            .rounds
            .iter()
            .find(|r| r.block == block)
            .map(|r| &r.authors)
            .ok_or(Error::InvalidSync)?
            .iter()
            .zip(seq.into_iter())
            .filter_map(|(author, seq)| seq.map(|seq| (*author, seq)))
            .collect();
        Ok(self.graph.sync(authors))
    }
}

impl<T: Serialize> Voter<T> {
    /// The maximum created round of all self parents of x (or 1 if there are none).
    /// Event x is a witness if x has a greater created round than its self parent.
    pub fn add_event<F: FnOnce() -> Result<(u64, Box<[Author]>), Error>>(
        &mut self,
        event: RawEvent<T>,
        start_round: F,
    ) -> Result<Hash, Error> {
        let parent = event.event.self_hash;
        let other_parent = event.event.other_hash;
        let hash = self.graph.add_event(event)?;

        let parent_round_num = parent
            .map(|h| self.graph.event(&h).unwrap().round_created().unwrap())
            .unwrap_or(0);
        let other_parent_round_num = other_parent
            .map(|h| self.graph.event(&h).unwrap().round_created().unwrap())
            .unwrap_or(0);
        let max_parent_round_num = u64::max(parent_round_num, other_parent_round_num);

        let parent_round = self.round(max_parent_round_num);

        let next_round = parent_round
            .map(|r| {
                let n_strongly_see = r
                    .witnesses()
                    .into_iter()
                    .filter(|w| self.graph.strongly_see(&hash, w, r.authors()))
                    .count();
                n_strongly_see > r.threshold()
            })
            .unwrap_or(true);

        let round_num = if next_round {
            max_parent_round_num + 1
        } else {
            max_parent_round_num
        };

        let is_witness = round_num > parent_round_num;

        if is_witness {
            if let Some(round) = self.round_mut(round_num) {
                round.witnesses.push(hash);
            } else {
                let (block, authors) = start_round()?;
                let mut round = Round::new(round_num, block, authors);
                round.witnesses.push(hash);
                self.rounds.push(round);
            }
        }

        let mut event = self.graph.event_mut(&hash).unwrap();
        event.round_created = Some(round_num);
        event.witness = Some(is_witness);
        Ok(hash)
    }
}

impl<T> Voter<T> {
    fn round(&self, round: u64) -> Option<&Round> {
        self.rounds.iter().find(|r| r.round == round)
    }

    fn round_mut(&mut self, round: u64) -> Option<&mut Round> {
        self.rounds.iter_mut().find(|r| r.round == round)
    }

    /// Decide if a witness is famous.
    fn is_witness_famous(&self, _witness: &Hash, rounds: &[Round]) -> Option<bool> {
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
    fn famous_witnesses(&self, rounds: &[Round]) -> Option<Vec<Hash>> {
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
