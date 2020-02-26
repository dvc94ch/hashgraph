use crate::error::StateError;
use crate::hash::{FileHasher, Hash};
use async_std::path::Path;
use async_std::prelude::*;
use core::ops::RangeBounds;

#[derive(Clone, Debug)]
pub enum Op {
    Insert(Box<[u8]>, Box<[u8]>),
    Remove(Box<[u8]>),
    CompareAndSwap(Box<[u8]>, Option<Box<[u8]>>, Option<Box<[u8]>>),
}

pub struct State(sled::Tree);

impl State {
    pub fn from_tree(tree: sled::Tree) -> Self {
        Self(tree)
    }

    pub fn commit(&self, op: Op) -> Result<(), StateError> {
        match op {
            Op::Insert(key, value) => {
                self.0.insert(&key, value)?;
            }
            Op::Remove(key) => {
                self.0.remove(&key)?;
            }
            Op::CompareAndSwap(key, old, new) => {
                self.0.compare_and_swap(&key, old, new)?.ok();
            }
        };
        Ok(())
    }

    pub fn tree(&self) -> Tree {
        Tree(self.0.clone())
    }

    pub async fn export_checkpoint(&mut self, dir: &Path) -> Result<Hash, StateError> {
        let mut fh = FileHasher::create_tmp(&dir).await?;
        let mut exporter = Exporter {
            tree: &self.0,
            fh: &mut fh,
        };
        exporter.write_tree().await?;
        Ok(fh.rename(&dir).await?)
    }

    pub async fn import_checkpoint(&mut self, dir: &Path, hash: &Hash) -> Result<(), StateError> {
        self.0.clear()?;
        let mut fh = FileHasher::open_with_hash(dir, hash).await?;
        let mut importer = Importer {
            tree: &self.0,
            fh: &mut fh,
        };
        importer.read_tree().await?;
        if fh.hash() != *hash {
            self.0.clear()?;
            return Err(StateError::InvalidCheckpoint);
        }
        Ok(())
    }
}

struct Exporter<'a> {
    tree: &'a sled::Tree,
    fh: &'a mut FileHasher,
}

impl<'a> Exporter<'a> {
    async fn write_len(&mut self, len: usize) -> Result<(), StateError> {
        let bytes = (len as u64).to_be_bytes();
        self.fh.write_all(&bytes).await?;
        Ok(())
    }

    async fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), StateError> {
        self.write_len(bytes.len()).await?;
        self.fh.write_all(bytes).await?;
        Ok(())
    }

    async fn write_tree(&mut self) -> Result<(), StateError> {
        self.write_len(self.tree.len()).await?;
        for entry in self.tree.iter() {
            let (k, v) = entry?;
            self.write_bytes(&k).await?;
            self.write_bytes(&v).await?;
        }
        Ok(())
    }
}

struct Importer<'a> {
    tree: &'a sled::Tree,
    fh: &'a mut FileHasher,
}

impl<'a> Importer<'a> {
    async fn read_len(&mut self) -> Result<usize, StateError> {
        let mut bytes = [0u8; 8];
        self.fh.read_exact(&mut bytes).await?;
        Ok(u64::from_be_bytes(bytes) as usize)
    }

    async fn read_bytes(&mut self) -> Result<Vec<u8>, StateError> {
        let len = self.read_len().await?;
        let mut key = vec![0u8; len];
        self.fh.read_exact(&mut key).await?;
        Ok(key)
    }

    async fn read_tree(&mut self) -> Result<(), StateError> {
        let len = self.read_len().await?;
        for _ in 0..len {
            let key = self.read_bytes().await?;
            let value = self.read_bytes().await?;
            self.tree.insert(key, value)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Tree(sled::Tree);

impl Tree {
    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<Option<sled::IVec>> {
        self.0.get(key)
    }

    pub fn watch_prefix<P: AsRef<[u8]>>(&self, prefix: P) -> sled::Subscriber {
        self.0.watch_prefix(prefix)
    }

    pub fn contains_key<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<bool> {
        self.0.contains_key(key)
    }

    pub fn get_lt<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<Option<(sled::IVec, sled::IVec)>> {
        self.0.get_lt(key)
    }

    pub fn get_gt<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<Option<(sled::IVec, sled::IVec)>> {
        self.0.get_gt(key)
    }

    pub fn iter(&self) -> sled::Iter {
        self.0.iter()
    }

    pub fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> sled::Iter {
        self.0.range(range)
    }

    pub fn scan_prefix<P: AsRef<[u8]>>(&self, prefix: P) -> sled::Iter {
        self.0.scan_prefix(prefix)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn name(&self) -> sled::IVec {
        self.0.name()
    }

    pub fn checksum(&self) -> sled::Result<u32> {
        self.0.checksum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    fn bx(b: &[u8]) -> Box<[u8]> {
        b.to_vec().into_boxed_slice()
    }

    #[test]
    fn test_commit() {
        let tmpdir = TempDir::new("test_commit").unwrap();
        let path: &Path = tmpdir.path().into();
        let db = sled::open(path).unwrap();
        let tree = db.open_tree("state").unwrap();
        let state = State::from_tree(tree);

        state.commit(Op::Insert(bx(b"key"), bx(b"value"))).unwrap();
        let tree = state.tree();
        let value = tree.get(b"key").unwrap();
        assert_eq!(value.as_ref().map(|v| v.as_ref()), Some(&b"value"[..]));
        state.commit(Op::Remove(bx(b"key"))).unwrap();
        assert_eq!(tree.get(b"key").unwrap(), None);
    }

    #[async_std::test]
    async fn test_export_import() {
        let tmpdir = TempDir::new("test_export_import").unwrap();
        let path: &Path = tmpdir.path().into();
        let db = sled::open(path.join("sled")).unwrap();
        let tree = db.open_tree("state").unwrap();
        let mut state = State::from_tree(tree);

        let dir = path.join("checkpoint");
        async_std::fs::create_dir_all(&dir).await.unwrap();
        state.commit(Op::Insert(bx(b"key"), bx(b"value"))).unwrap();
        let hash = state.export_checkpoint(&dir).await.unwrap();
        state.import_checkpoint(&dir, &hash).await.unwrap();
        let hash2 = state.export_checkpoint(&dir).await.unwrap();
        assert_eq!(hash, hash2);
    }
}
