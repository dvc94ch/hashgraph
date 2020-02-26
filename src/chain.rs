use crate::author::{Author, Signature};
use crate::error::StateError;
use crate::hash::{Hash, Hasher, GENESIS_HASH, HASH_LENGTH};
use disco::ed25519::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use std::collections::HashSet;

fn canonicalize_authors(set: &HashSet<Author>) -> Box<[Author]> {
    let mut authors = Vec::with_capacity(set.len());
    for author in set.iter() {
        authors.push(*author);
    }
    authors.sort();
    authors.into_boxed_slice()
}

pub struct BlockBuilder {
    parent: Hash,
    authors: HashSet<Author>,
}

impl BlockBuilder {
    pub fn genesis(authors: HashSet<Author>) -> Self {
        Self {
            authors,
            parent: GENESIS_HASH,
        }
    }

    pub fn len(&self) -> usize {
        self.authors.len()
    }

    pub fn insert(&mut self, author: Author) {
        self.authors.insert(author);
    }

    pub fn to_proposed(&mut self) -> ProposedBlock {
        let authors = canonicalize_authors(&self.authors);
        self.authors.clear();

        let mut hasher = Hasher::new();
        hasher.write(&*self.parent);
        for author in &authors[..] {
            hasher.write(author.as_bytes());
        }
        let hash = hasher.sum();
        let parent = std::mem::replace(&mut self.parent, hash);

        ProposedBlock {
            parent,
            authors,
            hash,
            signees: Default::default(),
            signatures: Default::default(),
        }
    }
}

pub struct ProposedBlock {
    parent: Hash,
    authors: Box<[Author]>,
    hash: Hash,
    signees: HashSet<Author>,
    signatures: Vec<Signature>,
}

impl ProposedBlock {
    pub fn add_sig(&mut self, author: Author, sig: Signature) -> bool {
        if self.signees.contains(&author) {
            return false;
        }
        if author.verify(&*self.hash, &sig).is_err() {
            return false;
        }
        self.signees.insert(author);
        self.signatures.push(sig);
        true
    }

    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    pub fn to_block(self) -> (Hash, Block) {
        let block = Block {
            parent: self.parent,
            authors: self.authors,
            signatures: self.signatures.into_boxed_slice(),
        };
        (self.hash, block)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    parent: Hash,
    authors: Box<[Author]>,
    signatures: Box<[Signature]>,
}

impl Block {
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            16 + HASH_LENGTH
                + PUBLIC_KEY_LENGTH * self.authors.len()
                + SIGNATURE_LENGTH * self.signatures.len(),
        );
        buf.extend(&*self.parent);
        buf.extend(&(self.authors.len() as u64).to_be_bytes());
        for author in &self.authors[..] {
            buf.extend(author.as_bytes());
        }
        buf.extend(&(self.signatures.len() as u64).to_be_bytes());
        for sig in &self.signatures[..] {
            buf.extend(&sig.to_bytes()[..]);
        }
        buf
    }

    fn deserialize(buf: &[u8]) -> Result<Self, StateError> {
        let mut i1 = 0;
        let mut i2 = HASH_LENGTH;
        let parent = Hash::from_bytes(&buf[i1..i2]);
        i1 = i2;
        i2 += 8;
        let mut bytes = [0u8; 8];
        bytes.clone_from_slice(&buf[i1..i2]);
        let len = u64::from_be_bytes(bytes) as usize;
        let mut authors = Vec::with_capacity(len);
        for _ in 0..len {
            i1 = i2;
            i2 += PUBLIC_KEY_LENGTH;
            authors.push(Author::from_bytes(&buf[i1..i2])?);
        }
        i1 = i2;
        i2 += 8;
        let mut bytes = [0u8; 8];
        bytes.clone_from_slice(&buf[i1..i2]);
        let len = u64::from_be_bytes(bytes) as usize;
        let mut signatures = Vec::with_capacity(len);
        for _ in 0..len {
            i1 = i2;
            i2 += SIGNATURE_LENGTH;
            signatures.push(Signature::from_bytes(&buf[i1..i2])?);
        }
        Ok(Self {
            parent,
            authors: authors.into_boxed_slice(),
            signatures: signatures.into_boxed_slice(),
        })
    }
}

pub struct AuthorChain {
    authors: HashSet<Author>,
    builder: BlockBuilder,
    proposed: Option<ProposedBlock>,
    tree: sled::Tree,
}

impl AuthorChain {
    pub fn genesis(tree: sled::Tree, genesis_authors: HashSet<Author>) -> Result<Self, StateError> {
        let mut builder = BlockBuilder::genesis(genesis_authors.clone());
        let proposed = builder.to_proposed();
        let (hash, block) = proposed.to_block();
        tree.insert(&*hash, block.serialize())?;

        Ok(Self {
            authors: genesis_authors,
            builder: builder,
            proposed: None,
            tree,
        })
    }

    //pub fn from_tree(tree: sled::Tree) -> Result<Self, StateError> {
    //}

    pub fn start_round(&mut self) -> Result<Box<[Author]>, StateError> {
        if let Some(proposed) = self.proposed.take() {
            let population = self.authors.len();
            let threshold = population - population * 2 / 3;
            if proposed.len() >= threshold {
                let (hash, block) = proposed.to_block();
                self.tree.insert(&*hash, block.serialize())?;
                for author in &block.authors[..] {
                    if self.authors.contains(author) {
                        self.authors.remove(author);
                    } else {
                        self.authors.insert(*author);
                    }
                }
            }
        }
        if self.builder.len() > 0 {
            self.proposed = Some(self.builder.to_proposed());
        }
        Ok(canonicalize_authors(&self.authors))
    }

    pub fn hash(&self) -> Option<Hash> {
        self.proposed.as_ref().map(|p| p.hash)
    }

    pub fn add_author(&mut self, author: Author) {
        if !self.authors.contains(&author) {
            self.builder.insert(author);
        }
    }

    pub fn rem_author(&mut self, author: Author) {
        if self.authors.contains(&author) {
            self.builder.insert(author);
        }
    }

    pub fn add_sig(&mut self, sig: Signature) {
        if let Some(proposed) = &mut self.proposed {
            for author in &self.authors {
                if proposed.add_sig(*author, sig) {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::Identity;
    use async_std::path::Path;
    use sled::Tree;
    use tempdir::TempDir;

    fn setup() -> (TempDir, Tree) {
        let tmpdir = TempDir::new("test_chain").unwrap();
        let path: &Path = tmpdir.path().into();
        let db = sled::open(path).unwrap();
        let tree = db.open_tree("author").unwrap();
        (tmpdir, tree)
    }

    #[test]
    fn block_serde() {
        let block = Block {
            parent: Hash::random(),
            authors: vec![Identity::generate().author(), Identity::generate().author()]
                .into_boxed_slice(),
            signatures: vec![
                Identity::generate().sign(&*Hash::random()),
                Identity::generate().sign(&*Hash::random()),
            ]
            .into_boxed_slice(),
        };
        let bytes = block.serialize();
        let block2 = Block::deserialize(&bytes).unwrap();
        assert_eq!(block, block2);
    }

    #[test]
    fn test_chain() {
        let (_tmpdir, tree) = setup();
        let id1 = Identity::generate();
        let id2 = Identity::generate();
        let id3 = Identity::generate();
        let mut authors = HashSet::new();
        authors.insert(id1.author());
        authors.insert(id2.author());
        authors.insert(id3.author());
        let mut chain = AuthorChain::genesis(tree, authors).unwrap();
        chain.add_author(Identity::generate().author());
        let authors = chain.start_round().unwrap();
        assert_eq!(authors.len(), 3);
        chain.add_sig(id1.sign(&*chain.hash().unwrap()));
        let authors = chain.start_round().unwrap();
        assert_eq!(authors.len(), 4);
    }
}
