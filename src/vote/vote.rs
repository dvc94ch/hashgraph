//! Implements voting and round handling.
use super::event::RawEvent;
use crate::author::Author;
use crate::error::Error;
use crate::hash::Hash;
use crate::vote::graph::Graph;
use serde::Serialize;
use std::collections::HashMap;

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
    /// If the fame of all witnesses is decided.
    decided: bool,
    /// Unique famous witnesses
    unique_famous_witnesses: Vec<Hash>,
}

impl Round {
    pub fn new(round: u64, block: u64, authors: Box<[Author]>) -> Self {
        let witnesses = Vec::with_capacity(authors.len());
        let unique_famous_witnesses = Vec::with_capacity(authors.len());
        Self {
            round,
            block,
            authors,
            witnesses,
            freq_coin_rounds: FREQ_COIN_ROUNDS,
            decided: false,
            unique_famous_witnesses,
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

    fn decide_round<T>(&mut self, graph: &Graph<T>) {
        self.decided = true;
        let mut authors = HashMap::new();
        for witness in self.witnesses() {
            let event = graph.event(witness).unwrap();
            let num_witnesses = authors.get(&event.author()).cloned().unwrap_or(0);
            authors.insert(event.author(), num_witnesses + 1);
        }
        for witness in &self.witnesses {
            let event = graph.event(witness).unwrap();
            let num_witnesses = *authors.get(&event.author()).unwrap();
            if num_witnesses > 1 {
                continue;
            }
            self.unique_famous_witnesses.push(*witness);
        }
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

    /// A round is decided when the fame of all it's witnesses is decided.
    fn decide_fame(&mut self, i: usize) -> bool {
        let round = &self.rounds[i];
        let threshold = round.threshold();
        let mut num_decided = 0;
        for witness in round.witnesses() {
            if self.graph.event(witness).unwrap().famous.is_some() {
                num_decided += 1;
                continue;
            }
            for (voter, round, diff) in WitnessIter::new(&self.rounds[i..]) {
                if diff == 1 {
                    // first round of the election
                    let vote = self.graph.see(voter, witness);
                    self.graph
                        .event_mut(voter)
                        .unwrap()
                        .votes
                        .insert(*witness, vote);
                } else {
                    let parent_round = self.round(round.round - 1).unwrap();
                    let strongly_seen_witnesses = parent_round
                        .witnesses()
                        .into_iter()
                        .filter(|w| self.graph.strongly_see(voter, w, parent_round.authors()));
                    // majority vote in strongly_seen_witnesses (is true for a tie)
                    // number of events in s with a vote of v
                    let (mut vote, num_votes) = {
                        let votes = strongly_seen_witnesses
                            .filter_map(|w| {
                                self.graph.event(w).unwrap().votes.get(witness).cloned()
                            })
                            .collect::<Vec<_>>();
                        let num_votes = votes.len();
                        let yes_votes = votes.into_iter().filter(|v| *v == true).count();
                        let no_votes = num_votes - yes_votes;
                        (yes_votes >= no_votes, usize::max(yes_votes, no_votes))
                    };

                    if num_votes <= threshold && diff % round.freq_coin_rounds() > 0 {
                        // this is a coin round so flip a coin
                        vote = self.graph.event(voter).unwrap().signature().to_bytes()[32] & 1 == 1
                    }

                    self.graph
                        .event_mut(voter)
                        .unwrap()
                        .votes
                        .insert(*witness, vote);
                    //println!("num_votes {}, threshold {}", num_votes, threshold);
                    if num_votes > threshold {
                        self.graph.event_mut(witness).unwrap().famous = Some(vote);
                        num_decided += 1;
                    }
                }
            }
        }
        //println!("round: {} num decided: {}", round.round, num_decided);
        num_decided >= round.authors().len()
    }

    /// Iterates through rounds and performs a vote. If the fame of all witnesses
    /// is decided it finalizes the round.
    pub fn process_rounds(&mut self) -> Vec<Hash> {
        //println!("process_rounds");
        let mut commit = Vec::new();
        for i in 0..self.rounds.len() {
            if self.rounds[i].decided {
                continue;
            }
            //println!("decide fame of round {}", self.rounds[i].round);
            if self.decide_fame(i) {
                let graph = &self.graph;
                let round = &mut self.rounds[i];
                round.decide_round(graph);
                let roots = round
                    .unique_famous_witnesses
                    .iter()
                    .map(|e| graph.event(e).unwrap())
                    .collect::<Vec<_>>();
                let hashes = graph
                    .shared_ancestors(&roots)
                    .filter(|e| e.round_received.is_none())
                    .map(|e| *e.hash())
                    .collect::<Vec<_>>();
                let mut whitener = roots[0].signature().to_bytes();
                for root in &roots[1..] {
                    whitener = xor(&whitener, &root.signature().to_bytes());
                }
                //println!("round decided");
                //println!("unique_famous_witnesses: {:?}", roots.len());
                //println!("shared ancestors: {:?}", hashes.len());
                for h in &hashes {
                    let round = &self.rounds[i];
                    let event = self.graph.event(h).unwrap();

                    let mut timestamps = Vec::with_capacity(round.unique_famous_witnesses.len());
                    for witness in &round.unique_famous_witnesses {
                        let witness = self.graph.event(witness).unwrap();
                        let timestamp = self
                            .graph
                            .self_ancestors(witness)
                            .find(|ancestor| {
                                let next_ancestor =
                                    ancestor.self_parent().map(|e| self.graph.event(e).unwrap());
                                self.graph.ancestor(ancestor, event)
                                    && next_ancestor
                                        .map(|ancestor| !self.graph.ancestor(ancestor, event))
                                        .unwrap_or(true)
                            })
                            .map(|e| e.time())
                            .unwrap();
                        timestamps.push(*timestamp);
                    }
                    timestamps.sort();
                    let time_received = timestamps[timestamps.len() / 2];
                    let whitened_signature = xor(&whitener, &event.signature().to_bytes());

                    let mut event = self.graph.event_mut(h).unwrap();
                    event.round_received = Some(round.round);
                    event.time_received = Some(time_received);
                    event.whitened_signature = Some(whitened_signature);
                    commit.push(*event.hash());
                }
            } else {
                break;
            }
        }
        let mut events = commit
            .into_iter()
            .map(|h| self.graph.event(&h).unwrap())
            .collect::<Vec<_>>();
        //println!("committing {} events", events.len());
        events.sort();
        events.into_iter().map(|e| *e.hash()).collect()
    }
}

struct WitnessIter<'a> {
    rounds: &'a [Round],
    ri: usize,
    wi: usize,
}

impl<'a> WitnessIter<'a> {
    pub fn new(rounds: &'a [Round]) -> Self {
        Self {
            rounds,
            ri: 1,
            wi: 0,
        }
    }
}

impl<'a> Iterator for WitnessIter<'a> {
    type Item = (&'a Hash, &'a Round, usize);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(round) = self.rounds.get(self.ri) {
                if let Some(witness) = round.witnesses.get(self.wi) {
                    self.wi += 1;
                    return Some((witness, &self.rounds[self.ri], self.ri));
                } else {
                    self.ri += 1;
                    self.wi = 0;
                    continue;
                }
            } else {
                return None;
            }
        }
    }
}

fn xor(x: &[u8; 64], y: &[u8; 64]) -> [u8; 64] {
    let mut n = [0; 64];
    for i in 0..x.len() {
        n[i] = x[i] ^ y[i];
    }
    n
}
