//! Syncing between peers.
use crate::event::{RawEvent, RawProperties};
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
pub struct Syncer<TEvent> {
    graph: Graph<TEvent>,
}

impl<TEvent: RawProperties> Syncer<TEvent> {
    /// Import an event received in a sync.
    pub fn import_event(&self, raw: RawEvent) {
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
