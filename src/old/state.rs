use crate::author::{Author, Identity};
use crate::checkpoint::{Checkpoint, StagingCheckpoint, SignedCheckpoint};
use crate::error::Error;
use crate::event::{Event, Payload};
use crate::graph::Graph;
use crate::hash::{Hash, Hasher};
use std::collections::{HashMap, HashSet};

/// Trait to be implemented by the db engine.
pub trait StateMachine {
    /// Commit a transaction.
    fn commit(&mut self, payload: &[u8]);

    /// Return the hash of the current state.
    fn state(&mut self) -> Hash;
}

pub struct BasicStateMachine(Hasher);

impl BasicStateMachine {
    pub fn new() -> Self {
        Self(Hasher::new())
    }
}

impl StateMachine for BasicStateMachine {
    fn commit(&mut self, payload: &[u8]) {
        self.0.write(payload);
    }

    fn state(&mut self) -> Hash {
        let hasher = std::mem::replace(&mut self.0, Hasher::new());
        let hash = hasher.sum();
        self.commit(&*hash);
        hash
    }
}

pub struct State<SM: StateMachine> {
    /// Identity.
    identity: Identity,
    /// State machine.
    state_machine: SM,
    /// List of authors in this round.
    authors: Vec<Author>,
    /// Event graph.
    graph: Graph,
    /// Transaction queue.
    queue: Vec<Payload>,
    /*
    /// Last agreed upon finalized checkpoint.
    checkpoint: SignedCheckpoint,

    /// List of authors in this round.
    authors: Vec<Author>,
    /// List of sequence numbers of events last seen for a given author.
    events: HashMap<Author, (u32, Hash)>,
    /// Last event hash
    last_event: Hash,
    /// Last seen event
    last_seen_event: Hash,


    /// Authors of the next round.
    staging_authors: HashSet<Author>,
    /// Next checkpoint.
    staging_checkpoint: StagingCheckpoint,*/
}

impl<SM: StateMachine> State<SM> {
    pub fn new(
        state_machine: SM,
        identity: Identity,
    ) -> Self {
        let authors = vec![identity.author()];
        Self {
            state_machine,
            identity,
            authors,
            graph: Graph::new(),
            queue: Vec::new(),
        }
    }

    pub fn add_author(&mut self, author: Author) {
        self.create_transaction(Payload::AddAuthor(author));
    }

    pub fn remove_author(&mut self, author: Author) {
        self.create_transaction(Payload::RemoveAuthor(author));
    }

    pub fn create_transaction(&mut self, payload: Payload) {
        self.queue.push(payload);
    }

    /*pub fn create_event(&mut self) -> Event {
        let payload = std::mem::replace(&mut self.queue, Vec::new());
        let author = self.authors.iter().position(|author| self.identity.author()).unwrap() as u32;
        let time = SystemTime::now();
        // use identity to create an event
        Event {
            payload,
            hashes: [self.last_event, self.last_seen_event],
            time,
            author,
            //hash: //,
            //signature: //,
            //seq: self.events,
            round: None,
            witness: None,
        }
        unimplemented!()
    }*/

    /*pub fn from_checkpoint(
        state_machine: SM,
        identity: Identity,
        checkpoint: SignedCheckpoint,
    ) -> Result<Self, Error> {
        if state_machine.state() != *checkpoint.state() {
            return Err(Error::InvalidState);
        }
        // TODO what if not an Author?
        let me = identity.author();
        let mut authors = Vec::new();
        let mut events = HashMap::new();
        let mut staging_authors = HashSet::new();
        let mut last_event = None;
        let mut last_seen_event = None;
        for (author, seq, event) in checkpoint.authors() {
            if *author == me {
                last_event = Some(event);
            } else {
                last_seen_event = Some(event);
            }
            authors.push(author.clone());
            events.insert(author.clone(), (*seq, event.clone()));
            staging_authors.insert(author.clone());
        }
        Ok(Self {
            identity,
            state_machine,
            checkpoint,

            authors,
            events,
            last_event: last_event.unwrap().clone(),
            last_seen_event: last_seen_event.unwrap().clone(),
            graph: Graph::new(),
            queue: Vec::new(),

            staging_authors,
            staging_checkpoint: None,
        })
    }





    pub fn import_event(&mut self, event: Event) {
    }

    /// Freezes the authors into a round and returns it's population.
    pub fn begin_round(&mut self) -> u32 {
        let mut round_authors: Vec<_> = self.authors.iter().map(Clone::clone).collect();
        round_authors.sort();
        let population = round_authors.len() as u32;
        self.authors = round_authors;
        population
    }

    pub fn commit_event(&mut self, event: &Event) {
        for payload in event.payload() {
            match payload {
                Payload::AddAuthor(author) => { self.staging_authors.insert(author.clone()); }
                Payload::RemoveAuthor(author) => { self.staging_authors.remove(author); }
                Payload::Checkpoint(sig) => { self.staging_checkpoint.add_sig(*sig); }
                Payload::Raw(payload) => { self.state_machine.commit(&**payload); }
            }
        }
    }

    pub fn end_round(&mut self) {
        let state = self.state_machine.state();
        let authors = self.staging_authors
            .iter()
            .map(|author| {
                let (seq, hash) = self.events.get(author).unwrap();
                (*author, *seq, *hash)
            })
            .collect();
        let cp = Checkpoint {
            state,
            authors,
        };
        let cp = StagingCheckpoint::new(cp);
        self.queue.push(Payload::Checkpoint(self.identity.sign(&**cp.hash())));
        self.staging_checkpoint = cp;
    }

    pub fn checkpoint(&self) -> &SignedCheckpoint {
        &self.checkpoint
    }*/
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state() {
        let sm = BasicStateMachine::new();
        let id1 = Identity::generate();
        let mut s1 = State::new(sm, id1);

        let id2 = Identity::generate();
        s1.add_author(id2.author());
        let e = s1.create_event();
        //s1.commit_event(e);

        //let sm = BasicStateMachine::from_checkpoint();
        //let mut s2 = State::new(
    }

    /*#[test]
    fn test_state_from_checkpoint() {
        let cp = Checkpoint {
            state: sm.state(),
            authors: vec![
                (id1, 3, Hash::random()),
                (id2, 4, Hash::random()),
                (id3, 1, Hash::random()),
            ],
        };
        let cp = SignedCheckpoint::new(cp);
        cp.add_sig(id1.author());
        cp.add_sig(id2.author());
        let cp = cp.into_signed_checkpoint();

        //let state = State::new(sm, id, cp).unwrap();
    }*/
}
