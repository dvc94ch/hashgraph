//! Implementation of the hashgrap aBFT consensus algorithm.
//#![deny(missing_docs)]
//#![deny(warnings)]
#![allow(dead_code)]
mod author;
mod error;
mod event;
mod graph;
mod hash;
mod state;
mod vote;

pub use crate::author::Author;
use crate::author::Identity;
pub use crate::error::Error;
use crate::event::UnsignedRawEvent;
pub use crate::event::{Event, RawEvent};
pub use crate::hash::Hash;
use crate::state::State;
pub use crate::state::{SignedCheckpoint, Transaction, Tree};
use crate::vote::Voter;
use async_std::fs;
use async_std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::time::SystemTime;

pub struct HashGraph {
    voter: Voter<Transaction>,
    state: State,
    identity: Identity,
    queue: Vec<Transaction>,
    self_hash: Option<Hash>,
    other_hash: Option<Hash>,
}

impl HashGraph {
    pub async fn open_default() -> Result<Self, Error> {
        let dir = dirs::config_dir().ok_or(Error::ConfigDir)?;
        let dir = PathBuf::from(dir);
        Self::open(&dir.join("hashgraph")).await
    }

    pub async fn open(dir: &Path) -> Result<Self, Error> {
        fs::create_dir_all(&dir).await?;
        let identity = Identity::load_from(&dir.join("identity")).await?;
        let mut state = State::open(dir)?;
        let (block, authors) = state.start_round()?;
        let voter = Voter::new(block, authors);
        Ok(Self {
            identity,
            state,
            voter,
            queue: Default::default(),
            self_hash: None,
            other_hash: None,
        })
    }

    pub fn genesis(&mut self, genesis_authors: HashSet<Author>) -> Result<(), Error> {
        self.state.genesis(genesis_authors)
    }

    pub fn create_transaction(&mut self, tx: Transaction) {
        self.queue.push(tx);
    }

    pub fn sync_state(&self) -> (u64, Box<[Option<u64>]>) {
        self.voter.sync_state()
    }

    pub fn outbound_sync(
        &self,
        state: (u64, Box<[Option<u64>]>),
    ) -> Result<Option<impl Iterator<Item = &RawEvent<Transaction>>>, Error> {
        self.voter.sync(state)
    }

    pub fn inbound_sync(
        &mut self,
        events: impl Iterator<Item = RawEvent<Transaction>>,
    ) -> Result<(), Error> {
        let identity = self.identity();
        let state = &mut self.state;

        // Import events.
        for event in events {
            let author = event.event.author;
            let hash = self.voter.add_event(event, || state.start_round())?;
            if author != identity {
                self.other_hash = Some(hash);
            }
        }

        // Create sync event.
        let payload = std::mem::replace(&mut self.queue, Vec::new()).into_boxed_slice();
        let time = SystemTime::now();
        let (hash, event) = UnsignedRawEvent {
            self_hash: self.self_hash.take(),
            other_hash: self.other_hash,
            payload,
            time,
            author: identity,
        }
        .sign(&self.identity)?;
        self.self_hash = Some(hash);
        self.voter.add_event(event, || state.start_round())?;

        // Process new events
        self.voter.process_rounds();
        Ok(())
    }

    pub fn state_tree(&self) -> Tree {
        self.state.state_tree()
    }

    pub fn identity(&self) -> Author {
        self.identity.author()
    }

    pub async fn import_checkpoint(
        &mut self,
        dir: &Path,
        checkpoint: SignedCheckpoint,
    ) -> Result<(), Error> {
        self.state.import_checkpoint(dir, checkpoint).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    async fn create_graphs(n: usize) -> Result<(Vec<TempDir>, Vec<Option<HashGraph>>), Error> {
        let mut tmp = Vec::with_capacity(n);
        let mut g = Vec::with_capacity(n);
        let mut authors = HashSet::new();
        for i in 0..n {
            tmp.push(TempDir::new(&format!("hashgraph{}", n))?);
            let graph = HashGraph::open(tmp[i].path().into()).await?;
            authors.insert(graph.identity());
            g.push(Some(graph));
        }
        for i in 0..n {
            g[i].as_mut().unwrap().genesis(authors.clone())?;
        }
        Ok((tmp, g))
    }

    fn sync(g1: &mut HashGraph, g2: &HashGraph) {
        let state = g1.sync_state();
        let res = if let Some(iter) = g2.outbound_sync(state).unwrap() {
            g1.inbound_sync(iter.map(|r| r.clone()))
        } else {
            g1.inbound_sync(core::iter::empty())
        };
        if let Err(err) = res {
            println!("{:#?}", g1.voter.graph());
            panic!("{}", err);
        }
    }

    #[allow(unused_variables)]
    #[async_std::test]
    async fn consensus() {
        let (_tmp, mut g) = create_graphs(4).await.unwrap();
        let mut a = g[0].take().unwrap();
        let mut b = g[1].take().unwrap();
        let mut c = g[2].take().unwrap();
        let mut d = g[3].take().unwrap();

        /* D1.1 -> B1.0 */
        sync(&mut d, &b);
        /* B1.1 -> D1.1 */
        sync(&mut b, &d);
        /* D1.2 -> B1.1 */
        sync(&mut d, &b);
        /* B1.2 -> C1.0 */
        sync(&mut b, &c);
        /* A1.1 -> B1.1 */
        sync(&mut a, &b);
        /* D1.3 -> B1.2 */
        sync(&mut d, &b);
        /* C1.1 -> B1.2 */
        sync(&mut c, &b);
        /* B1.3 -> D1.3 */
        sync(&mut b, &d);
        /* D2.0 -> A1.1 */
        sync(&mut d, &a);
        /* A2.0 -> D2.0 */
        sync(&mut a, &d);
        /* B2.0 -> D2.0 */
        sync(&mut b, &d);
        /* A2.1 -> C1.1 */
        sync(&mut a, &c);
        /* A2.2 -> B2.0 */
        sync(&mut a, &b);
        /* C2.0 -> A2.1 */
        sync(&mut c, &a);
        /* D2.1 -> B2.0 */
        sync(&mut d, &b);
        /* D2.2 -> A2.2 */
        sync(&mut d, &a);
        /* B2.1 -> A2.2 */
        sync(&mut b, &a);
        /* B3.0 -> D2.2 */
        sync(&mut b, &d);
        /* A3.0 -> B3.0 */
        sync(&mut a, &b);
        /* D3.0 -> B3.0 */
        sync(&mut d, &b);
        /* B3.1 -> A3.0 */
        sync(&mut b, &a);
        /* A3.1 -> B3.1 */
        sync(&mut a, &b);
        /* D3.1 -> C2.0 */
        sync(&mut d, &c);
        /* C3.0 -> D3.1 */
        sync(&mut c, &d);
        /* B3.2 -> D3.1 */
        sync(&mut b, &d);
        /* A3.2 -> B3.2 */
        sync(&mut a, &b);
        /* D3.2 -> B3.2 */
        sync(&mut d, &b);
        /* B3.3 -> A3.2 */
        sync(&mut b, &a);
        /* D4.0 -> C3.0 */
        sync(&mut d, &c);
        /* B4.0 -> D4.0 */
        sync(&mut b, &d);
    }
}
