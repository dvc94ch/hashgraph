//! Author tracking.
use multihash::{Multihash, MultihashDigest};
use ed25519_dalek as ed25519;
use std::collections::{HashMap, HashSet};

/// Public key of an author.
pub type Public = Box<[u8]>;

/// Author registry.
pub struct Authors {
    authors: HashSet<Multihash>,
    rounds: Vec<Vec<Multihash>>,
    public_keys: HashMap<Multihash, Public>,
}

impl Authors {
    /// Add an author.
    pub fn add_author(&mut self, public: Public) {
        let peer_id = multihash::Sha2_256::digest(&public);
        self.public_keys.insert(peer_id.clone(), public);
        self.authors.insert(peer_id);
    }

    /// Remove an author.
    pub fn remove_author(&mut self, peer_id: &Multihash) {
        self.authors.remove(peer_id);
    }

    /// Freezes the authors into a round.
    pub fn start_round(&mut self) {
        let mut round_authors: Vec<_> = self.authors.iter().map(Clone::clone).collect();
        round_authors.sort();
        self.rounds.push(round_authors);
    }

    /// Gets the number of authors in a round.
    pub fn population(&self, round: u32) -> u32 {
        self.rounds[round as usize].len() as u32
    }
}
