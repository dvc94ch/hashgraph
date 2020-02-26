use crate::author::{Author, Signature};
use crate::hash::Hash;
use core::ops::Deref;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Checkpoint(pub(crate) Hash);

impl Deref for Checkpoint {
    type Target = Hash;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedCheckpoint {
    pub checkpoint: Checkpoint,
    pub signatures: Box<[Signature]>,
}

impl Deref for SignedCheckpoint {
    type Target = Hash;

    fn deref(&self) -> &Self::Target {
        &*self.checkpoint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProposedCheckpoint {
    checkpoint: Checkpoint,
    signees: HashSet<Author>,
    signatures: Vec<Signature>,
}

impl ProposedCheckpoint {
    pub fn new(checkpoint: Checkpoint) -> Self {
        Self {
            checkpoint,
            signees: Default::default(),
            signatures: Default::default(),
        }
    }

    pub fn add_sig(&mut self, author: Author, sig: Signature) {
        if self.signees.contains(&author) {
            return;
        }
        if author.verify(&**self.checkpoint, &sig).is_err() {
            return;
        }
        self.signees.insert(author);
        self.signatures.push(sig);
    }

    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    pub fn into_signed_checkpoint(self) -> SignedCheckpoint {
        SignedCheckpoint {
            checkpoint: self.checkpoint,
            signatures: self.signatures.into_boxed_slice(),
        }
    }
}

impl Deref for ProposedCheckpoint {
    type Target = Hash;

    fn deref(&self) -> &Self::Target {
        &*self.checkpoint
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::Identity;

    #[test]
    fn test_checkpoint() {
        let id1 = Identity::generate();
        let id2 = Identity::generate();
        let checkpoint = Checkpoint(Hash::random());

        let mut proof = ProposedCheckpoint::new(checkpoint.clone());
        proof.add_sig(id1.author(), id1.sign(&**proof));
        proof.add_sig(id2.author(), id2.sign(&**proof));
        proof.add_sig(id2.author(), id2.sign(&**proof));
        proof.add_sig(id1.author(), id2.sign(&**proof));
        assert_eq!(proof.len(), 2);
    }
}
