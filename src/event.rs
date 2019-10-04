//! Defines an event and it's properties.
use multihash::Multihash;
use std::time::SystemTime;

/// A public key.
pub type Public = Box<[u8]>;

/// A digital signature.
pub type Signature = Box<[u8]>;

/// Raw properties of an event.
pub trait RawProperties {
    /// Payload of the event.
    fn payload(&self) -> &[u8];

    /// Set of hashes of parents of an event.
    fn parent_hashes(&self) -> &Vec<Multihash>;

    /// Hash of the self parent of an event.
    fn self_parent_hash(&self) -> Option<&Multihash>;

    /// Author's claimed data and time of the event.
    fn time(&self) -> &SystemTime;

    /// Author of the event.
    fn author(&self) -> u32;

    /// Hash of the event.
    fn hash(&self) -> &Multihash;

    /// Signature of the event.
    fn signature(&self) -> &Signature;
}

/// Derived properties of an event.
pub trait DerivedProperties: RawProperties {
    /// Sequence number of the event.
    fn seq(&self) -> u32;

    /// Round of the event.
    fn round(&self) -> u32;

    /// First event of a new round.
    fn witness(&self) -> bool;
}

/// A raw event.
#[derive(Clone, Debug)]
pub struct RawEvent {
    /// Arbitrary binary payload of the event.
    payload: Box<[u8]>,
    /// A list of hashes of the event's parents, self-parent first.
    hashes: Vec<Multihash>,
    /// Author's claimed date and time of the event.
    time: SystemTime,
    /// Author id of the author.
    author: u32,
    /// Hash of {payload, hashes, time, author}.
    hash: Multihash,
    /// Author's digital signature of hash.
    signature: Signature,
}

impl RawProperties for RawEvent {
    fn payload(&self) -> &[u8] {
        &self.payload
    }

    fn parent_hashes(&self) -> &Vec<Multihash> {
        &self.hashes
    }

    fn self_parent_hash(&self) -> Option<&Multihash> {
        self.hashes.get(0)
    }

    fn time(&self) -> &SystemTime {
        &self.time
    }

    fn author(&self) -> u32 {
        self.author
    }

    fn hash(&self) -> &Multihash {
        &self.hash
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }
}

/// An event.
#[derive(Clone, Debug)]
pub struct Event {
    /// Raw event received.
    pub(crate) raw: RawEvent,
    /// Sequence number of the event.
    pub(crate) seq: u32,
    /// Round of the event.
    pub(crate) round: u32,
    /// Is first event of a new round.
    pub(crate) witness: bool,
}

impl RawProperties for Event {
    fn payload(&self) -> &[u8] {
        self.raw.payload()
    }

    fn parent_hashes(&self) -> &Vec<Multihash> {
        self.raw.parent_hashes()
    }

    fn self_parent_hash(&self) -> Option<&Multihash> {
        self.raw.self_parent_hash()
    }

    fn time(&self) -> &SystemTime {
        self.raw.time()
    }

    fn author(&self) -> u32 {
        self.raw.author()
    }

    fn hash(&self) -> &Multihash {
        self.raw.hash()
    }

    fn signature(&self) -> &Signature {
        self.raw.signature()
    }
}

impl DerivedProperties for Event {
    fn seq(&self) -> u32 {
        self.seq
    }

    fn round(&self) -> u32 {
        self.round
    }

    fn witness(&self) -> bool {
        self.witness
    }
}
