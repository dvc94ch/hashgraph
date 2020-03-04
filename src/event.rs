//! Defines an event and it's properties.
use crate::author::{Author, Identity, Signature};
use crate::error::Error;
use crate::hash::{Hash, Hasher, GENESIS_HASH};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

/// An unsigned raw hashgraph event.
#[derive(Clone, Debug)]
pub struct UnsignedRawEvent<T> {
    /// Arbitrary binary payload of the event.
    pub payload: Box<[T]>,
    /// The last self parent.
    pub self_hash: Option<Hash>,
    /// Last seen not self event hash.
    pub other_hash: Option<Hash>,
    /// Author's claimed date and time of the event.
    pub time: SystemTime,
    /// Author id of the author.
    pub author: Author,
}

impl<T: Serialize> UnsignedRawEvent<T> {
    pub fn hash(&self) -> Result<Hash, Error> {
        let mut hasher = Hasher::new();
        hasher.write(&*self.self_hash.unwrap_or(GENESIS_HASH));
        hasher.write(&*self.other_hash.unwrap_or(GENESIS_HASH));
        hasher.write(self.author.as_bytes());
        hasher.write(
            &self
                .time
                .duration_since(UNIX_EPOCH)?
                .as_nanos()
                .to_be_bytes(),
        );
        for p in &self.payload[..] {
            hasher.write(&bincode::serialize(p)?);
        }
        Ok(hasher.sum())
    }

    pub fn sign(self, identity: &Identity) -> Result<(Hash, RawEvent<T>), Error> {
        let hash = self.hash()?;
        let signature = identity.sign(&*hash);
        Ok((
            hash,
            RawEvent {
                event: self,
                signature,
            },
        ))
    }
}

/// A raw hashgraph event.
#[derive(Clone, Debug)]
pub struct RawEvent<T> {
    /// The raw event data.
    pub(crate) event: UnsignedRawEvent<T>,
    /// Author's digital signature of hash.
    pub(crate) signature: Signature,
}

/// A hashgraph event.
#[derive(Clone, Debug)]
pub struct Event<T> {
    /// Raw event
    pub(crate) raw: RawEvent<T>,
    /// Hash of {payload, hashes, time, author}.
    hash: Hash,
    /// Monotonically increasing sequence number of event.
    seq: u64,
    /// A list of hashes of the event's parents, self-parent first.
    parents: Vec<Hash>,
    /// A list of hashes of the event's children.
    children: Vec<Hash>,
    /// The round the event was created.
    pub(crate) round_created: Option<u64>,
    /// Is first event of a new round.
    pub(crate) witness: Option<bool>,
    /// Is the witness famous.
    pub(crate) famous: Option<bool>,
    /// The round the event was received.
    pub(crate) round_received: Option<u64>,
    /// The consensus timestamp of the event.
    pub(crate) time_received: Option<SystemTime>,
}

impl<T: Serialize> Event<T> {
    /// Create a new event from a raw event.
    pub(crate) fn new(raw: RawEvent<T>, hash: Hash, seq: u64) -> Self {
        let mut parents = Vec::with_capacity(2);
        if let Some(self_hash) = raw.event.self_hash {
            parents.push(self_hash);
        }
        if let Some(other_hash) = raw.event.other_hash {
            parents.push(other_hash);
        }
        Self {
            raw,
            hash,
            seq,
            parents,
            children: vec![],
            round_created: None,
            witness: None,
            famous: None,
            round_received: None,
            time_received: None,
        }
    }
}

impl<T> Event<T> {
    /// Payload of the event.
    pub fn payload(&self) -> &[T] {
        &self.raw.event.payload
    }

    /// Set of hashes of parents of an event.
    pub fn parents(&self) -> &[Hash] {
        &self.parents
    }

    /// Hash of the self parent of an event.
    pub fn self_parent(&self) -> Option<&Hash> {
        self.raw.event.self_hash.as_ref()
    }

    /// Set of hashes of children of an event.
    pub fn children(&self) -> &[Hash] {
        &self.children
    }

    /// Add a child hash.
    pub fn add_child(&mut self, hash: Hash) {
        self.children.push(hash);
    }

    /// Author's claimed data and time of the event.
    pub fn time(&self) -> &SystemTime {
        &self.raw.event.time
    }

    /// Author of the event.
    pub fn author(&self) -> Author {
        self.raw.event.author
    }

    /// Hash of the event.
    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    /// Signature of the event.
    pub fn signature(&self) -> &Signature {
        &self.raw.signature
    }

    /// Monotonically increasing sequence number of event.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Round the event belongs to.
    pub fn round_created(&self) -> Option<u64> {
        self.round_created
    }

    /// Is it the first event of a round.
    pub fn witness(&self) -> Option<bool> {
        self.witness
    }
}
