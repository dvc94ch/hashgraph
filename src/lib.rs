//! Implementation of the hashgrap aBFT consensus algorithm.
//#![deny(missing_docs)]
//#![deny(warnings)]
#![allow(dead_code)]
mod author;
mod error;
mod hash;
mod state;
mod vote;

pub use crate::author::Author;
use crate::author::Identity;
pub use crate::error::Error;
pub use crate::hash::Hash;
use crate::state::State;
pub use crate::state::{Key, SignedCheckpoint, Transaction, Tree, Value};
pub use crate::vote::RawEvent;
use crate::vote::{UnsignedRawEvent, Voter};
use async_std::fs;
use async_std::path::{Path, PathBuf};
use std::collections::HashSet;
use std::time::SystemTime;

pub struct HashGraph {
    voter: Voter<Transaction>,
    state: State,
    identity: Identity,
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

    pub fn outbound_sync(
        &self,
        state: (u64, Box<[Option<u64>]>),
    ) -> Result<impl Iterator<Item = &RawEvent<Transaction>>, Error> {
        self.voter.sync(state)
    }

    pub fn inbound_sync(
        &mut self,
        events: impl Iterator<Item = RawEvent<Transaction>>,
    ) -> Result<Hash, Error> {
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
        let payload = state.create_payload();
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
        let hash = self.voter.add_event(event, || state.start_round())?;

        // Process new events
        for hash in self.voter.process_rounds() {
            //println!("commit: {:?}", hash);
            let event = self.voter.graph().event(&hash).unwrap();
            let author = event.author();
            for payload in event.payload() {
                //println!("commit: {:?}", payload);
                self.state.commit(author, payload)?;
            }
            self.state.flush()?;
        }
        Ok(hash)
    }

    pub fn tree(&self) -> Tree {
        self.state.tree()
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
        g.sort_by(|a, b| {
            a.as_ref()
                .unwrap()
                .identity()
                .cmp(&b.as_ref().unwrap().identity())
        });
        for i in 0..n {
            g[i].as_mut().unwrap().genesis(authors.clone())?;
        }
        Ok((tmp, g))
    }

    fn sync_check(
        _authors: &[Author],
        g1: &mut HashGraph,
        g2: &HashGraph,
        n: &mut u64,
        round: u64,
        witness: bool,
    ) -> Hash {
        g1.tree()
            .insert(
                g1.identity().to_bytes(),
                b"seq",
                Value::new(n.to_be_bytes()),
            )
            .unwrap();
        *n += 1;
        let state = g1.sync_state();
        //println!("{:?} -> {:?}", state.1, g2.sync_state().1);
        //g1.voter.graph().display(authors);
        //println!("");
        //g2.voter.graph().display(authors);
        //println!("");
        let iter = g2.outbound_sync(state).unwrap();
        let hash = g1.inbound_sync(iter.map(|r| r.clone())).unwrap();

        //g1.voter.graph().display(authors);
        //println!("");

        //println!("{:#?}", g1.voter.rounds());
        let event = g1.voter.graph().event(&hash).expect("hash is in graph");
        let event_round = event.round_created().unwrap();
        let event_witness = event.witness().unwrap();
        if (event_round, event_witness) != (round, witness) {
            assert_eq!((event_round, event_witness), (round, witness));
        }
        hash
    }

    fn check_key(g: &HashGraph, author: &Author, value: u64) {
        let key = Key::new(author.to_bytes(), b"seq").unwrap();
        assert_eq!(
            g.tree().get(&key).unwrap(),
            Some(value.to_be_bytes().to_vec().into()),
        );
    }

    #[allow(unused_variables)]
    #[async_std::test]
    async fn consensus() {
        let (_tmp, mut g) = create_graphs(4).await.unwrap();
        let authors: Vec<_> = g.iter().map(|g| g.as_ref().unwrap().identity()).collect();
        let mut a = g[0].take().unwrap();
        let mut b = g[1].take().unwrap();
        let mut c = g[2].take().unwrap();
        let mut d = g[3].take().unwrap();
        let mut an = 1;
        let mut bn = 1;
        let mut cn = 1;
        let mut dn = 1;

        /* A1.0 -> Genesis */
        let a1 = a.inbound_sync(core::iter::empty()).unwrap();
        /* B1.0 -> Genesis */
        let b1 = b.inbound_sync(core::iter::empty()).unwrap();
        /* C1.0 -> Genesis */
        let c1 = c.inbound_sync(core::iter::empty()).unwrap();
        /* D1.0 -> Genesis */
        let d1 = d.inbound_sync(core::iter::empty()).unwrap();
        /* D1.1 -> B1.0 */
        sync_check(&authors, &mut d, &b, &mut dn, 1, false);
        /* B1.1 -> D1.1 */
        sync_check(&authors, &mut b, &d, &mut bn, 1, false);
        /* D1.2 -> B1.1 */
        sync_check(&authors, &mut d, &b, &mut dn, 1, false);
        /* A1.1 -> B1.1 */
        sync_check(&authors, &mut a, &b, &mut an, 1, false);
        /* B1.2 -> C1.0 */
        sync_check(&authors, &mut b, &c, &mut bn, 1, false);
        /* D1.3 -> B1.2 */
        sync_check(&authors, &mut d, &b, &mut dn, 1, false);
        /* C1.1 -> B1.2 */
        sync_check(&authors, &mut c, &b, &mut cn, 1, false);
        /* B1.3 -> D1.3 */
        sync_check(&authors, &mut b, &d, &mut bn, 1, false);
        /* D2.0 -> A1.1 */
        let d2 = sync_check(&authors, &mut d, &a, &mut dn, 2, true);
        /* A2.0 -> D2.0 */
        let a2 = sync_check(&authors, &mut a, &d, &mut an, 2, true);
        /* B2.0 -> D2.0 */
        let b2 = sync_check(&authors, &mut b, &d, &mut bn, 2, true);
        /* A2.1 -> C1.1 */
        sync_check(&authors, &mut a, &c, &mut an, 2, false);
        /* A2.2 -> B2.0 */
        sync_check(&authors, &mut a, &b, &mut an, 2, false);
        /* C2.0 -> A2.1 */
        let c2 = sync_check(&authors, &mut c, &a, &mut cn, 2, true);
        /* D2.1 -> B2.0 */
        sync_check(&authors, &mut d, &b, &mut dn, 2, false);
        /* D2.2 -> A2.2 */
        sync_check(&authors, &mut d, &a, &mut dn, 2, false);
        /* B2.1 -> A2.2 */
        sync_check(&authors, &mut b, &a, &mut bn, 2, false);
        /* B3.0 -> D2.2 */
        sync_check(&authors, &mut b, &d, &mut bn, 3, true);
        /* A3.0 -> B3.0 */
        sync_check(&authors, &mut a, &b, &mut an, 3, true);
        /* D3.0 -> B3.0 */
        sync_check(&authors, &mut d, &b, &mut dn, 3, true);
        /* B3.1 -> A3.0 */
        sync_check(&authors, &mut b, &a, &mut bn, 3, false);
        /* A3.1 -> B3.1 */
        sync_check(&authors, &mut a, &b, &mut an, 3, false);
        /* D3.1 -> C2.0 */
        sync_check(&authors, &mut d, &c, &mut dn, 3, false);
        /* C3.0 -> D3.1 */
        sync_check(&authors, &mut c, &d, &mut cn, 3, true);
        /* B3.2 -> D3.1 */
        sync_check(&authors, &mut b, &d, &mut bn, 3, false);
        /* A3.2 -> B3.2 */
        sync_check(&authors, &mut a, &b, &mut an, 3, false);
        /* D3.2 -> B3.2 */
        sync_check(&authors, &mut d, &b, &mut dn, 3, false);
        /* B3.3 -> A3.2 */
        sync_check(&authors, &mut b, &a, &mut bn, 3, false);
        /* D4.0 -> C3.0 */
        sync_check(&authors, &mut d, &c, &mut dn, 4, true);
        /* B4.0 -> D4.0 */
        sync_check(&authors, &mut b, &d, &mut bn, 4, true);

        let graphs = [
            //a.voter.graph(),
            b.voter.graph(),
            //c.voter.graph(),
            d.voter.graph(),
        ];
        let famous = [
            (a1, true),
            (b1, true),
            (c1, true),
            (d1, true),
            (a2, true),
            (b2, true),
            (c2, false),
            (d2, true),
        ];
        for (i, (h, f)) in famous.iter().enumerate() {
            for g in &graphs {
                assert_eq!(g.event(h).unwrap().famous, Some(*f));
            }
        }

        check_key(&b, &a.identity(), 1);
        check_key(&b, &b.identity(), 2);
        check_key(&b, &d.identity(), 4);

        check_key(&d, &a.identity(), 1);
        check_key(&d, &b.identity(), 2);
        check_key(&d, &d.identity(), 4);
    }
}
