use crate::author::{Author, Signature};
use crate::error::Error;
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

fn lookup(hash: &Hash) -> Vec<u8> {
    let mut key = Vec::with_capacity(HASH_LENGTH + 8);
    key.extend(b"lookup::");
    key.extend(&(&*hash)[..]);
    key
}

#[derive(Debug, Eq, PartialEq)]
pub struct Block {
    parent: Hash,
    authors: Box<[Author]>,
}

impl Block {
    pub fn new(parent: Hash, authors: Box<[Author]>) -> Self {
        Self { parent, authors }
    }

    pub fn hash(&self) -> Hash {
        let mut hasher = Hasher::new();
        hasher.write(&*self.parent);
        for author in &self.authors[..] {
            hasher.write(author.as_bytes());
        }
        hasher.sum()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SignedBlock {
    block: Block,
    signatures: Box<[Signature]>,
}

impl SignedBlock {
    pub fn new(block: Block, signatures: Box<[Signature]>) -> Self {
        Self { block, signatures }
    }

    pub fn validate_and_apply(self, authors: &mut HashSet<Author>) -> Result<Vec<u8>, Error> {
        let population = authors.len();
        let threshold = population - population * 2 / 3;
        let hash = self.block.hash();
        let mut signees = HashSet::new();
        for sig in &self.signatures[..] {
            for author in authors.iter() {
                if signees.contains(author) {
                    continue;
                }
                if author.verify(&*hash, sig).is_err() {
                    continue;
                }
                signees.insert(*author);
            }
        }
        if signees.len() < threshold {
            return Err(Error::InvalidBlock);
        }
        for author in &self.block.authors[..] {
            if authors.contains(author) {
                authors.remove(author);
            } else {
                authors.insert(*author);
            }
        }
        Ok(self.serialize())
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(
            16 + HASH_LENGTH
                + PUBLIC_KEY_LENGTH * self.block.authors.len()
                + SIGNATURE_LENGTH * self.signatures.len(),
        );
        buf.extend(&*self.block.parent);
        buf.extend(&(self.block.authors.len() as u64).to_be_bytes());
        for author in &self.block.authors[..] {
            buf.extend(author.as_bytes());
        }
        buf.extend(&(self.signatures.len() as u64).to_be_bytes());
        for sig in &self.signatures[..] {
            buf.extend(&sig.to_bytes()[..]);
        }
        buf
    }

    pub fn deserialize(buf: &[u8]) -> Result<Self, Error> {
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
        let block = Block::new(parent, authors.into_boxed_slice());
        Ok(Self::new(block, signatures.into_boxed_slice()))
    }
}

pub struct BlockBuilder {
    parent: Hash,
    authors: HashSet<Author>,
}

impl BlockBuilder {
    pub fn new(parent: Hash) -> Self {
        Self {
            parent,
            authors: Default::default(),
        }
    }

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
        let block = Block::new(self.parent, authors);

        let proposed = ProposedBlock::new(block);
        self.parent = proposed.hash;
        proposed
    }
}

pub struct ProposedBlock {
    block: Block,
    hash: Hash,
    signees: HashSet<Author>,
    signatures: Vec<Signature>,
}

impl ProposedBlock {
    pub fn new(block: Block) -> Self {
        Self {
            hash: block.hash(),
            block,
            signees: Default::default(),
            signatures: Default::default(),
        }
    }

    pub fn add_sig(&mut self, author: Author, sig: Signature) {
        if self.signees.contains(&author) {
            return;
        }
        if author.verify(&*self.hash, &sig).is_err() {
            return;
        }
        self.signees.insert(author);
        self.signatures.push(sig);
    }

    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    pub fn into_signed_block(self) -> (Hash, SignedBlock) {
        let block = SignedBlock {
            block: self.block,
            signatures: self.signatures.into_boxed_slice(),
        };
        (self.hash, block)
    }
}

pub struct AuthorChain {
    pub(crate) tree: sled::Tree,
    authors: HashSet<Author>,
    builder: BlockBuilder,
    proposed: Option<ProposedBlock>,
    block: u64,
}

impl AuthorChain {
    pub fn from_tree(tree: sled::Tree) -> Result<Self, Error> {
        let mut lookup_hash = GENESIS_HASH;
        let mut block_id = 0;
        let mut authors = HashSet::new();
        loop {
            if let Some(block_hash) = tree.get(lookup(&lookup_hash))? {
                lookup_hash = Hash::from_bytes(&block_hash);
                if let Some(bytes) = tree.get(&*lookup_hash)? {
                    let block = SignedBlock::deserialize(&bytes)?;
                    if block.validate_and_apply(&mut authors).is_err() {
                        return Err(Error::InvalidState);
                    }
                    block_id += 1;
                } else {
                    return Err(Error::InvalidState);
                };
            } else {
                break;
            }
        }
        Ok(Self {
            authors,
            builder: BlockBuilder::new(lookup_hash),
            proposed: None,
            tree,
            block: block_id,
        })
    }

    pub fn genesis(&mut self, genesis_authors: HashSet<Author>) -> Result<(), Error> {
        self.builder = BlockBuilder::genesis(genesis_authors.clone());
        let proposed = self.builder.to_proposed();
        let (hash, block) = proposed.into_signed_block();
        self.tree.clear()?;
        self.tree.insert(&*hash, block.serialize())?;
        self.tree.insert(lookup(&GENESIS_HASH), &*hash)?;
        self.authors = genesis_authors;
        self.block = 1;
        Ok(())
    }

    pub fn start_round(&mut self) -> Result<Box<[Author]>, Error> {
        if let Some(proposed) = self.proposed.take() {
            let population = self.authors.len();
            let threshold = population - population * 2 / 3;
            if proposed.len() >= threshold {
                let (hash, block) = proposed.into_signed_block();
                let parent = block.block.parent;
                if let Ok(bytes) = block.validate_and_apply(&mut self.authors) {
                    self.tree.insert(&*hash, bytes)?;
                    self.tree.insert(lookup(&parent), &*hash)?;
                    self.block += 1;
                }
            }
        }
        if self.builder.len() > 0 {
            self.proposed = Some(self.builder.to_proposed());
        }
        Ok(canonicalize_authors(&self.authors))
    }

    pub fn genesis_hash(&self) -> Result<Hash, Error> {
        if let Some(hash) = self.tree.get(lookup(&GENESIS_HASH))? {
            Ok(Hash::from_bytes(&hash))
        } else {
            Err(Error::InvalidState)
        }
    }

    pub fn hash(&self) -> Option<Hash> {
        self.proposed.as_ref().map(|p| p.hash)
    }

    pub fn add_author(&mut self, author: Author, block: u64) {
        if self.block != block {
            return;
        }
        if !self.authors.contains(&author) {
            self.builder.insert(author);
        }
    }

    pub fn rem_author(&mut self, author: Author, block: u64) {
        if self.block != block {
            return;
        }
        if self.authors.contains(&author) {
            self.builder.insert(author);
        }
    }

    pub fn sign_block(&mut self, author: Author, sig: Signature) {
        if let Some(proposed) = &mut self.proposed {
            proposed.add_sig(author, sig);
        }
    }

    pub fn authors(&self) -> &HashSet<Author> {
        &self.authors
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
        let tree = db.open_tree("authors").unwrap();
        (tmpdir, tree)
    }

    #[test]
    fn block_serde() {
        let block = SignedBlock {
            block: Block {
                parent: Hash::random(),
                authors: vec![Identity::generate().author(), Identity::generate().author()]
                    .into_boxed_slice(),
            },
            signatures: vec![
                Identity::generate().sign(&*Hash::random()),
                Identity::generate().sign(&*Hash::random()),
            ]
            .into_boxed_slice(),
        };
        let bytes = block.serialize();
        let block2 = SignedBlock::deserialize(&bytes).unwrap();
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
        let mut chain = AuthorChain::from_tree(tree.clone()).unwrap();
        chain.genesis(authors).unwrap();
        chain.add_author(Identity::generate().author(), 1);
        chain.add_author(Identity::generate().author(), 2);
        let authors = chain.start_round().unwrap();
        assert_eq!(authors.len(), 3);
        chain.sign_block(id1.author(), id1.sign(&*chain.hash().unwrap()));
        let authors = chain.start_round().unwrap();
        assert_eq!(authors.len(), 4);
        let genesis = chain.genesis_hash().unwrap();

        let mut chain = AuthorChain::from_tree(tree).unwrap();
        assert_eq!(chain.genesis_hash().unwrap(), genesis);
        let authors2 = chain.start_round().unwrap();
        assert_eq!(authors, authors2);
    }
}
