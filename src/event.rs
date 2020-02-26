//! Defines an event and it's properties.
use crate::author::Signature;
use crate::hash::Hash;
use std::time::SystemTime;

/// A hashgraph event.
#[derive(Clone, Debug)]
pub struct Event {
    /// Arbitrary binary payload of the event.
    pub(crate) payload: Box<[u8]>,
    /// A list of hashes of the event's parents, self-parent first.
    pub(crate) hashes: [Option<Hash>; 2],
    /// Author's claimed date and time of the event.
    pub(crate) time: SystemTime,
    /// Author id of the author.
    pub(crate) author: u32,
    /// Hash of {payload, hashes, time, author}.
    pub(crate) hash: Hash,
    /// Author's digital signature of hash.
    pub(crate) signature: Signature,
    /// Monotonically increasing sequence number of event.
    pub(crate) seq: u32,
    /// Round of the event.
    pub(crate) round: Option<u32>,
    /// Is first event of a new round.
    pub(crate) witness: Option<bool>,
}

impl Event {
    /// Payload of the event.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Set of hashes of parents of an event.
    pub fn parent_hashes(&self) -> Vec<&Hash> {
        self.hashes.iter().filter_map(|h| h.as_ref()).collect()
    }

    /// Hash of the self parent of an event.
    pub fn self_parent_hash(&self) -> Option<&Hash> {
        self.hashes[0].as_ref()
    }

    /// Author's claimed data and time of the event.
    pub fn time(&self) -> &SystemTime {
        &self.time
    }

    /// Author of the event.
    pub fn author(&self) -> u32 {
        self.author
    }

    /// Hash of the event.
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Signature of the event.
    pub fn signature(&self) -> &Signature {
        &self.signature
    }

    /// Monotonically increasing sequence number of event.
    pub fn seq(&self) -> u32 {
        self.seq
    }

    /// Round the event belongs to.
    pub fn round(&self) -> Option<u32> {
        self.round
    }

    /// Is it the first event of a round.
    pub fn witness(&self) -> Option<bool> {
        self.witness
    }
}
