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

    pub fn sync_state(&self) -> (u64, Box<[Option<u64>]>) {
        self.voter.sync_state()
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
        Ok(&self.voter.graph().event(&hash).unwrap().raw)
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

    async fn create_graphs(n: usize) -> Result<(Vec<TempDir>, Vec<HashGraph>), Error> {
        let mut tmp = Vec::with_capacity(n);
        let mut g = Vec::with_capacity(n);
        let mut authors = HashSet::new();
        for i in 0..n {
            tmp.push(TempDir::new(&format!("hashgraph{}", n))?);
            g.push(HashGraph::open(tmp[i].path().into()).await?);
            authors.insert(g[i].identity());
        }
        for i in 0..n {
            g[i].genesis(authors.clone())?;
        }
        Ok((tmp, g))
    }

    fn link(g: &mut HashGraph, event: &RawEvent<Transaction>) -> RawEvent<Transaction> {
        if let Err(err) = g.add_event(event.clone()) {
            println!("{:#?}", g.voter.graph());
            println!("{:#?}", event);
            panic!("{}", err);
        }
        g.create_event().unwrap().clone()
    }

    #[allow(unused_variables)]
    #[async_std::test]
    async fn consensus() {
        let (_tmp, mut g) = create_graphs(4).await.unwrap();

        let a1 = g[0].create_event().unwrap().clone();
        let b1 = g[1].create_event().unwrap().clone();
        let c1 = g[2].create_event().unwrap().clone();
        let d1 = g[3].create_event().unwrap().clone();

        let d11 = link(&mut g[3], &b1);
        let b11 = link(&mut g[1], &d11);
        let a11 = link(&mut g[0], &b11);
        let b12 = link(&mut g[1], &c1);
        let d12 = link(&mut g[3], &b11);
        let d13 = link(&mut g[3], &b12);
        let b13 = link(&mut g[1], &d13);
        let c11 = link(&mut g[2], &b12);

        let d2 = link(&mut g[3], &a11);
        let a2 = link(&mut g[0], &d2);
        let a21 = link(&mut g[0], &c11);
        let b2 = link(&mut g[1], &d2);
        let c2 = link(&mut g[2], &a21);
        let a22 = link(&mut g[0], &b2);
        let d21 = link(&mut g[3], &b2);
        let d22 = link(&mut g[3], &a22);
        let b21 = link(&mut g[1], &a22);

        let b3 = link(&mut g[1], &d22);
        let a3 = link(&mut g[0], &b3);
        let b31 = link(&mut g[1], &a3);
        let b32 = link(&mut g[1], &a3);
        let d3 = link(&mut g[3], &b3);
        let d31 = link(&mut g[3], &c2);
        let b33 = link(&mut g[1], &d31);
        let a31 = link(&mut g[0], &b32);
        let a32 = link(&mut g[0], &b33);
        let b34 = link(&mut g[1], &a32);
        let c3 = link(&mut g[2], &d31);
        let d32 = link(&mut g[3], &b33);
        let d4 = link(&mut g[3], &c3);
    }
}
