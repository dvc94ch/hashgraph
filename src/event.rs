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

    /// Monotonically increasing sequence number of event.
    fn seq(&self) -> u32;
}

/// A raw event.
#[derive(Clone, Debug)]
pub struct RawEvent {
    /// Arbitrary binary payload of the event.
    pub(crate) payload: Box<[u8]>,
    /// A list of hashes of the event's parents, self-parent first.
    pub(crate) hashes: Vec<Multihash>,
    /// Author's claimed date and time of the event.
    pub(crate) time: SystemTime,
    /// Author id of the author.
    pub(crate) author: u32,
    /// Hash of {payload, hashes, time, author}.
    pub(crate) hash: Multihash,
    /// Author's digital signature of hash.
    pub(crate) signature: Signature,
    /// Monotonically increasing sequence number of event.
    pub(crate) seq: u32,
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

    fn seq(&self) -> u32 {
        self.seq
    }
}

/// Derived properties of an event.
pub trait DerivedProperties: RawProperties {
    /// Round of the event.
    fn round(&self) -> u32;

    /// First event of a new round.
    fn witness(&self) -> bool;
}

/// An event.
#[derive(Clone, Debug)]
pub struct Event {
    /// Raw event received.
    pub(crate) raw: RawEvent,
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

    fn seq(&self) -> u32 {
        self.raw.seq()
    }
}

impl DerivedProperties for Event {
    fn round(&self) -> u32 {
        self.round
    }

    fn witness(&self) -> bool {
        self.witness
    }
}

impl<'a> From<&'a Event> for &'a RawEvent {
    fn from(event: &'a Event) -> Self {
        &event.raw
    }
}
