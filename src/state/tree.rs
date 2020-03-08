//! Tree utils.
use crate::error::Error;
use crate::hash::FileHasher;
use async_std::prelude::*;
use core::ops::RangeBounds;

#[derive(Clone, Debug)]
pub struct Tree(sled::Tree);

impl Tree {
    pub fn new(tree: sled::Tree) -> Self {
        Self(tree)
    }

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

pub struct Exporter<'a> {
    tree: &'a sled::Tree,
    fh: &'a mut FileHasher,
}

impl<'a> Exporter<'a> {
    pub fn new(tree: &'a sled::Tree, fh: &'a mut FileHasher) -> Self {
        Self { tree, fh }
    }

    async fn write_len(&mut self, len: usize) -> Result<(), Error> {
        let bytes = (len as u64).to_be_bytes();
        self.fh.write_all(&bytes).await?;
        Ok(())
    }

    async fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.write_len(bytes.len()).await?;
        self.fh.write_all(bytes).await?;
        Ok(())
    }

    pub async fn write_tree(&mut self) -> Result<(), Error> {
        self.write_len(self.tree.len()).await?;
        for entry in self.tree.iter() {
            let (k, v) = entry?;
            self.write_bytes(&k).await?;
            self.write_bytes(&v).await?;
        }
        Ok(())
    }
}

pub struct Importer<'a> {
    tree: &'a sled::Tree,
    fh: &'a mut FileHasher,
}

impl<'a> Importer<'a> {
    pub fn new(tree: &'a sled::Tree, fh: &'a mut FileHasher) -> Self {
        Self { tree, fh }
    }

    async fn read_len(&mut self) -> Result<usize, Error> {
        let mut bytes = [0u8; 8];
        self.fh.read_exact(&mut bytes).await?;
        Ok(u64::from_be_bytes(bytes) as usize)
    }

    async fn read_bytes(&mut self) -> Result<Vec<u8>, Error> {
        let len = self.read_len().await?;
        let mut key = vec![0u8; len];
        self.fh.read_exact(&mut key).await?;
        Ok(key)
    }

    pub async fn read_tree(&mut self) -> Result<(), Error> {
        let len = self.read_len().await?;
        for _ in 0..len {
            let key = self.read_bytes().await?;
            let value = self.read_bytes().await?;
            self.tree.insert(key, value)?;
        }
        Ok(())
    }
}
