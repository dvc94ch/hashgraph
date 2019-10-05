//! Implementation of the hashgrap aBFT consensus algorithm.
#![deny(missing_docs)]
#![deny(warnings)]

pub mod event;
pub mod graph;
pub mod round;
pub mod sync;

use async_std::stream::Stream;
use event::RawEvent;
use multihash::Multihash;
use sync::SyncState;

/// Trait to be implemented by the db engine.
pub trait StateMachine {
    /// Commit a transaction.
    fn commit(&mut self, payload: Box<[u8]>);

    /// Return the hash of the current state.
    fn checkpoint(&self) -> Multihash;
}

/// Consensus trait.
pub trait Consensus {
    /// Initialize consensus algorithm.
    fn new<T: StateMachine>(state: T, genesis: SyncState) -> Self;

    /// Create a transaction.
    fn create_transaction(payload: Box<[u8]>) -> dyn Stream<Item = TransactionEvent>;

    /// Import an event.
    fn import_event(&mut self, event: RawEvent);

    /// Get the graph state.
    fn get_state(&self) -> SyncState;

    /// Return the list of events known by the graph which are unknown by the
    /// state.
    fn diff_state<'a>(&'a self, state: SyncState) -> dyn Iterator<Item = &'a RawEvent>;
}

/// Transaction event.
pub enum TransactionEvent {
    /// Transaction was gossiped to another peer.
    Sync,
    /// Transaction was included in round.
    Round(u32),
    /// Transaction has been finalised with round received.
    Final(u32),
}
