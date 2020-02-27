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
        let state = State::open(dir)?;
        let voter = Voter::new();
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

    pub fn add_event(&mut self, event: RawEvent<Transaction>) -> Result<(), Error> {
        let state = &mut self.state;
        let author = event.event.author;
        let hash = self.voter.add_event(event, || state.start_round())?;
        if author != self.identity() {
            self.other_hash = Some(hash);
        }
        Ok(())
    }

    pub fn create_transaction(&mut self, tx: Transaction) {
        self.queue.push(tx);
    }

    pub fn create_event(&mut self) -> Result<&RawEvent<Transaction>, Error> {
        let payload = std::mem::replace(&mut self.queue, Vec::new()).into_boxed_slice();
        let time = SystemTime::now();
        let author = self.identity();
        let (hash, event) = UnsignedRawEvent {
            self_hash: self.self_hash.take(),
            other_hash: self.other_hash,
            payload,
            time,
            author,
        }
        .sign(&self.identity)?;
        self.self_hash = Some(hash);
        self.add_event(event)?;
        Ok(&self.voter.graph.event(&hash).unwrap().raw)
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

    #[async_std::test]
    async fn consensus() -> Result<(), Error> {
        let tmp1 = TempDir::new("hashgraph1")?;
        let mut g1 = HashGraph::open(tmp1.path().into()).await?;
        let tmp2 = TempDir::new("hashgraph2")?;
        let mut g2 = HashGraph::open(tmp2.path().into()).await?;
        let mut authors = HashSet::new();
        authors.insert(g1.identity());
        authors.insert(g2.identity());
        g1.genesis(authors.clone())?;
        g2.genesis(authors)?;

        g1.create_transaction(Transaction::insert(b"hello", b"world"));
        let event = g1.create_event()?.clone();
        g2.add_event(event)?;

        g2.create_transaction(Transaction::insert(b"world", b"hello"));
        let event = g2.create_event()?.clone();
        g1.add_event(event)?;

        Ok(())
    }
}
