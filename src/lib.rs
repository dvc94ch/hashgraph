//! Implementation of the hashgrap aBFT consensus algorithm.
//#![deny(missing_docs)]
//#![deny(warnings)]

mod author;
mod error;
mod event;
mod graph;
mod hash;
mod state;
mod vote;

use crate::author::Identity;
pub use crate::author::Author;
pub use crate::error::Error;
pub use crate::event::RawEvent;
use crate::vote::Voter;
use crate::state::State;
pub use crate::state::{Op, Tree};
use async_std::fs;
use async_std::path::{Path, PathBuf};
use std::collections::HashSet;

pub struct HashGraph {
    voter: Voter<Op>,
    state: State,
    identity: Identity,
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
        Ok(Self { identity, state, voter })
    }

    pub fn genesis(&mut self, genesis_authors: HashSet<Author>) -> Result<(), Error> {
        self.state.genesis(genesis_authors)
    }

    pub fn add_event(&mut self, event: RawEvent<Op>) -> Result<(), Error> {
        let state = &mut self.state;
        self.voter.add_event(event, || {
            state.start_round()
        })
    }

    pub fn state_tree(&self) -> Tree {
        self.state.state_tree()
    }
}
