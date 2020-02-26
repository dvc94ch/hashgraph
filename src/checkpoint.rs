use crate::author::{Author, Signature};
use crate::error::StateError;
use crate::hash::{Hash, Hasher};
use core::ops::Deref;
use std::collections::HashSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedCheckpoint {
    inner: Checkpoint,
    sigs: Box<[Signature]>,
}

impl Deref for SignedCheckpoint {
    type Target = Checkpoint;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagingCheckpoint {
    inner: Checkpoint,
    hash: Hash,
    signees: HashSet<Author>,
    sigs: Vec<Signature>,
}

impl StagingCheckpoint {
    pub fn new(checkpoint: Checkpoint) -> Self {
        let hash = checkpoint.hash();
        Self {
            inner: checkpoint,
            hash,
            signees: Default::default(),
            sigs: Default::default(),
        }
    }

    pub fn hash(&self) -> Hash {
        self.hash
    }

    fn check_sig(&self, sig: &Signature) -> Option<Author> {
        for (author, _, _) in self.inner.authors() {
            if author.verify(&*self.hash, &sig).is_ok() {
                return Some(author.clone());
            }
        }
        None
    }

    pub fn add_sig(&mut self, sig: Signature) {
        if let Some(author) = self.check_sig(&sig) {
            if !self.signees.contains(&author) {
                self.signees.insert(author);
                self.sigs.push(sig);
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        self.sigs.len() as u32 >= self.inner.threshold()
    }

    pub fn into_signed_checkpoint(self) -> Result<SignedCheckpoint, StateError> {
        if !self.is_valid() {
            return Err(StateError::InvalidCheckpoint);
        }
        Ok(SignedCheckpoint {
            inner: self.inner,
            sigs: self.sigs.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Checkpoint {
    state: Hash,
    authors: Vec<(Author, u32, Hash)>,
}

impl Checkpoint {
    fn hash(&self) -> Hash {
        let mut hasher = Hasher::new();
        hasher.write(&*self.state);
        for (author, seq, event) in &self.authors {
            hasher.write(author.as_bytes());
            hasher.write(&seq.to_be_bytes());
            hasher.write(&**event);
        }
        hasher.sum()
    }

    // at least 2/3 are honest so with 1/3 valid signatures
    // at least one honest node signed
    fn threshold(&self) -> u32 {
        let population = self.authors.len() as u32;
        population - population * 2 / 3
    }

    pub fn state(&self) -> &Hash {
        &self.state
    }

    pub fn authors(&self) -> &[(Author, u32, Hash)] {
        &self.authors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::Identity;

    #[test]
    fn test_checkpoint_valid() {
        let id1 = Identity::generate();
        let id2 = Identity::generate();
        let id3 = Identity::generate();
        let id4 = Identity::generate();
        let id5 = Identity::generate();
        let checkpoint = Checkpoint {
            state: Hash::random(),
            authors: vec![
                (id1.author(), 3, Hash::random()),
                (id2.author(), 4, Hash::random()),
                (id3.author(), 1, Hash::random()),
                (id4.author(), 1, Hash::random()),
            ],
        };

        let mut proof = StagingCheckpoint::new(checkpoint.clone());
        let hash = proof.hash();
        proof.add_sig(id1.sign(&*hash));
        proof.add_sig(id2.sign(&*hash));
        assert!(proof.is_valid());

        let mut proof = StagingCheckpoint::new(checkpoint.clone());
        proof.add_sig(id1.sign(&*hash));
        proof.add_sig(id1.sign(&*hash));
        assert!(!proof.is_valid());

        let mut proof = StagingCheckpoint::new(checkpoint);
        proof.add_sig(id1.sign(&*hash));
        proof.add_sig(id5.sign(&*hash));
        assert!(!proof.is_valid());
    }
}
