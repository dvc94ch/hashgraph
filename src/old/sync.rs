//! Syncing between peers.
use crate::event::{Event, Payload};
use crate::graph::Graph;

/// State of gossip graph.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyncState {
    seq: Vec<u32>,
}

impl SyncState {
    /// Genesis state.
    pub fn genesis(population: u32) -> Self {
        Self {
            seq: vec![0; population as usize],
        }
    }

    /// Size of the population
    pub fn population(&self) -> u32 {
        self.seq.len() as u32
    }
}

/// Syncs the gossip graph with other nodes.
pub struct Syncer {
    tx_queue: Vec<Payload>,
}

impl Syncer {
    pub fn create_transaction(&mut self,
    /// Import an event received in a sync.
    pub fn import_event(&self, raw: Event) {
        if !raw
            .parent_hashes()
            .iter()
            .all(|mh| self.graph.get(mh).is_some())
        {
            // TODO log error
            return;
        }
        // TODO check raw.hash()
        // TODO check raw.signature()
        let _seq = if let Some(parent) = self.graph.self_parent(&raw) {
            parent.seq() + 1
        } else {
            1
        };
    }
}
