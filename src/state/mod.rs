mod acl;
mod chain;
mod checkpoint;
mod serde;
mod tree;

use self::acl::Acl;
use self::chain::AuthorChain;
use self::serde::{Exporter, Importer};
pub use self::tree::Tree;
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
}

pub struct State {
    _db: sled::Db,
    authors: AuthorChain,
    state: Acl,
}

impl State {
    pub fn open(path: &Path) -> Result<Self, StateError> {
        let db = sled::open(path.join("sled"))?;
        let authors = AuthorChain::from_tree(db.open_tree("authors")?)?;
        let state = Acl::from_tree(db.open_tree("state")?);
        Ok(Self { _db: db, authors, state })
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
        }
        Ok(())
    }

    pub fn start_round(&mut self) -> Result<Box<[Author]>, StateError> {
        self.authors.start_round()
    }

    pub fn block_hash(&self) -> Option<Hash> {
        self.authors.hash()
    }

    pub async fn export_checkpoint(&mut self, dir: &Path) -> Result<Hash, StateError> {
        let mut fh = FileHasher::create_tmp(&dir).await?;
        Exporter::new(&self.authors.tree, &mut fh)
            .write_tree()
            .await?;
        Exporter::new(&self.state.tree, &mut fh)
            .write_tree()
            .await?;
        Ok(fh.rename(&dir).await?)
    }

    pub async fn import_checkpoint(&mut self, dir: &Path, hash: &Hash) -> Result<(), StateError> {
        self.authors.tree.clear()?;
        self.state.tree.clear()?;
        let mut fh = FileHasher::open_with_hash(dir, hash).await?;
        Importer::new(&self.authors.tree, &mut fh)
            .read_tree()
            .await?;
        Importer::new(&self.state.tree, &mut fh).read_tree().await?;
        if fh.hash() != *hash {
            self.authors.tree.clear()?;
            self.state.tree.clear()?;
            return Err(StateError::InvalidCheckpoint);
        }
        Ok(())
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

        let hash = state.export_checkpoint(&dir).await.unwrap();
        state.import_checkpoint(&dir, &hash).await.unwrap();
        let hash2 = state.export_checkpoint(&dir).await.unwrap();
        assert_eq!(hash, hash2);
    }
}
