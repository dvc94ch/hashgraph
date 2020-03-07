//! Defines an event and it's properties.
use crate::author::{Author, Identity, Signature};
use crate::error::Error;
use crate::hash::{Hash, Hasher, GENESIS_HASH};
use core::cmp::Ordering;
use disco::ed25519::SIGNATURE_LENGTH;
use serde::Serialize;
use std::collections::HashMap;
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
#[derive(Clone)]
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
    /// Votes
    pub(crate) votes: HashMap<Hash, bool>,
    /// Is the witness famous.
    pub(crate) famous: Option<bool>,
    /// The round the event was received.
    pub(crate) round_received: Option<u64>,
    /// The consensus timestamp of the event.
    pub(crate) time_received: Option<SystemTime>,
    /// The whitened signature of the event.
    pub(crate) whitened_signature: Option<[u8; SIGNATURE_LENGTH]>,
}

impl<T> core::fmt::Debug for Event<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Event")
            .field("self_hash", &self.raw.event.self_hash)
            .field("other_hash", &self.raw.event.other_hash)
            .field("time", &self.raw.event.time)
            .field("author", &self.raw.event.author)
            .field("hash", &self.hash)
            .field("seq", &self.seq)
            .field("round_created", &self.round_created)
            .field("witness", &self.witness)
            .field("famous", &self.famous)
            .field("round_received", &self.round_received)
            .field("time_received", &self.time_received)
            .finish()
    }
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
            votes: Default::default(),
            famous: None,
            round_received: None,
            time_received: None,
            whitened_signature: None,
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
    pub fn author(&self) -> &Author {
        &self.raw.event.author
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

impl<T> PartialEq for Event<T> {
    fn eq(&self, other: &Event<T>) -> bool {
        self.signature() == other.signature()
    }
}

impl<T> Eq for Event<T> {}

impl<T> PartialOrd for Event<T> {
    fn partial_cmp(&self, other: &Event<T>) -> Option<Ordering> {
        if let (Some(rr1), Some(rr2)) = (self.round_received, other.round_received) {
            if rr1 != rr2 {
                return Some(rr1.cmp(&rr2));
            }
        } else {
            return None;
        }
        if let (Some(tr1), Some(tr2)) = (self.time_received, other.time_received) {
            if tr1 != tr2 {
                return Some(tr1.cmp(&tr2));
            }
        } else {
            return None;
        }
        if let (Some(wsig1), Some(wsig2)) = (self.whitened_signature, other.whitened_signature) {
            Some(wsig1.cmp(&wsig2))
        } else {
            None
        }
    }
}

impl<T> Ord for Event<T> {
    fn cmp(&self, other: &Event<T>) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}
