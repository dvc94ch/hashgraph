mod acl;
mod chain;
mod checkpoint;
mod serde;
mod tree;

use self::acl::Acl;
use self::chain::AuthorChain;
use self::checkpoint::ProposedCheckpoint;
use self::serde::{Exporter, Importer};
pub use self::tree::Tree;
pub use self::checkpoint::{Checkpoint, SignedCheckpoint};
use crate::author::{Author, Signature};
use crate::error::StateError;
use crate::hash::{FileHasher, Hash};
use async_std::path::Path;
use std::collections::HashSet;

#[derive(Debug)]
pub enum Op {
    AddAuthor(Author, u64),
    RemAuthor(Author, u64),
    SignBlock(Signature),
    Insert(Box<[u8]>, Box<[u8]>),
    Remove(Box<[u8]>),
    CompareAndSwap(Box<[u8]>, Option<Box<[u8]>>, Option<Box<[u8]>>),
    SignCheckpoint(Signature),
}

pub struct State {
    _db: sled::Db,
    authors: AuthorChain,
    state: Acl,
    checkpoint: Option<SignedCheckpoint>,
    proposed: Option<ProposedCheckpoint>,
}

impl State {
    pub fn open(path: &Path) -> Result<Self, StateError> {
        let db = sled::open(path.join("sled"))?;
        let authors = AuthorChain::from_tree(db.open_tree("authors")?)?;
        let state = Acl::from_tree(db.open_tree("state")?);
        Ok(Self { _db: db, authors, state, checkpoint: None, proposed: None })
    }

    pub fn genesis(&mut self, genesis_authors: HashSet<Author>) -> Result<(), StateError> {
        self.authors.genesis(genesis_authors)
    }

    pub fn genesis_hash(&self) -> Result<Hash, StateError> {
        self.authors.genesis_hash()
    }

    pub fn state_tree(&self) -> Tree {
        self.state.tree()
    }

    pub fn commit(&mut self, author: Author, op: &Op) -> Result<(), StateError> {
        match op {
            Op::AddAuthor(author, block) => self.authors.add_author(*author, *block),
            Op::RemAuthor(author, block) => self.authors.rem_author(*author, *block),
            Op::SignBlock(signature) => self.authors.sign_block(author, *signature),
            Op::Insert(key, value) => self.state.insert(author, key, value)?,
            Op::Remove(key) => self.state.remove(author, key)?,
            Op::CompareAndSwap(key, old, new) => {
                self.state.compare_and_swap(
                    author,
                    key,
                    old.as_ref().map(|b| &**b),
                    new.as_ref().map(|b| &**b),
                )?;
            }
            Op::SignCheckpoint(signature) => self.sign_checkpoint(author, *signature),
        }
        Ok(())
    }

    pub fn start_round(&mut self) -> Result<Box<[Author]>, StateError> {
        self.authors.start_round()
    }

    pub fn block_hash(&self) -> Option<Hash> {
        self.authors.hash()
    }

    pub async fn export_checkpoint(&mut self, dir: &Path) -> Result<Checkpoint, StateError> {
        let mut fh = FileHasher::create_tmp(&dir).await?;
        Exporter::new(&self.authors.tree, &mut fh)
            .write_tree()
            .await?;
        Exporter::new(&self.state.tree, &mut fh)
            .write_tree()
            .await?;
        let checkpoint = Checkpoint(fh.rename(&dir).await?);
        self.proposed = Some(ProposedCheckpoint::new(checkpoint));
        Ok(checkpoint)
    }

    pub async fn import_checkpoint(
        &mut self,
        dir: &Path,
        checkpoint: SignedCheckpoint,
    ) -> Result<(), StateError> {
        let genesis = self.genesis_hash().ok();

        self.authors.tree.clear()?;
        self.state.tree.clear()?;
        let mut fh = FileHasher::open_with_hash(dir, &*checkpoint).await?;
        Importer::new(&self.authors.tree, &mut fh)
            .read_tree()
            .await?;
        Importer::new(&self.state.tree, &mut fh).read_tree().await?;
        if fh.hash() != *checkpoint {
            self.authors.tree.clear()?;
            self.state.tree.clear()?;
            return Err(StateError::InvalidCheckpoint);
        }

        // make sure that it's still the same chain by comparing the new genesis hash.
        let new_authors = AuthorChain::from_tree(self.authors.tree.clone())?;
        if let Some(genesis) = genesis {
            let new_genesis = new_authors.genesis_hash()?;
            if genesis != new_genesis {
                return Err(StateError::InvalidCheckpoint);
            }
        }

        // check the signatures
        let population = new_authors.authors().len();
        let threshold = population - population * 2 / 3;
        let mut signees = HashSet::new();
        for sig in &checkpoint.signatures[..] {
            for author in new_authors.authors().iter() {
                if signees.contains(author) {
                    continue;
                }
                if author.verify(&**checkpoint, sig).is_err() {
                    continue;
                }
                signees.insert(*author);
            }
        }
        if signees.len() < threshold {
            return Err(StateError::InvalidCheckpoint);
        }

        self.authors = new_authors;
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    pub fn checkpoint(&self) -> Option<&SignedCheckpoint> {
        self.checkpoint.as_ref()
    }

    fn sign_checkpoint(&mut self, author: Author, sig: Signature) {
        if let Some(mut proposed) = self.proposed.take() {
            proposed.add_sig(author, sig);
            let population = self.authors.authors().len();
            let threshold = population - population * 2 / 3;
            if proposed.len() >= threshold {
                self.checkpoint = Some(proposed.into_signed_checkpoint());
            } else {
                self.proposed = Some(proposed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::author::Identity;
    use tempdir::TempDir;

    fn bx(b: &[u8]) -> Box<[u8]> {
        b.to_vec().into_boxed_slice()
    }

    fn gen_ids(n: usize) -> Vec<Identity> {
        let mut ids = Vec::with_capacity(n);
        for _ in 0..n {
            ids.push(Identity::generate());
        }
        ids
    }

    fn set(ids: &[Identity]) -> HashSet<Author> {
        let mut set = HashSet::new();
        for id in ids {
            set.insert(id.author());
        }
        set
    }

    #[test]
    fn test_authors() {
        let ids = gen_ids(4);
        let tmpdir = TempDir::new("test_authors").unwrap();
        let path: &Path = tmpdir.path().into();
        let mut state = State::open(path).unwrap();
        state.genesis(set(&ids[..2])).unwrap();

        let authors = state.start_round().unwrap();
        assert_eq!(authors.len(), 2);
        state.commit(ids[0].author(), &Op::AddAuthor(ids[2].author(), 1)).unwrap();
        state.commit(ids[0].author(), &Op::RemAuthor(ids[0].author(), 1)).unwrap();

        let authors2 = state.start_round().unwrap();
        assert_eq!(authors, authors2);
        let sig = ids[0].sign(&*state.block_hash().unwrap());
        state.commit(ids[0].author(), &Op::SignBlock(sig)).unwrap();

        let authors3 = state.start_round().unwrap();
        assert_eq!(authors3.len(), 2);
        assert_ne!(authors3, authors);
    }

    #[async_std::test]
    async fn test_export_import() {
        let ids = gen_ids(2);
        let tmpdir = TempDir::new("test_export_import").unwrap();
        let path: &Path = tmpdir.path().into();
        let mut state = State::open(path).unwrap();
        state.genesis(set(&ids)).unwrap();

        let dir = path.join("checkpoint");
        async_std::fs::create_dir_all(&dir).await.unwrap();
        state.commit(ids[0].author(), &Op::Insert(bx(b"key"), bx(b"value"))).unwrap();

        let checkpoint = state.export_checkpoint(&dir).await.unwrap();

        let signed = SignedCheckpoint {
            checkpoint,
            signatures: vec![ids[0].sign(&**checkpoint)].into_boxed_slice(),
        };
        state.import_checkpoint(&dir, signed).await.unwrap();

        let checkpoint2 = state.export_checkpoint(&dir).await.unwrap();
        assert_eq!(checkpoint, checkpoint2);
    }
}
