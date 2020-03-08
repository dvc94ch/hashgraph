//! Tree utils.
use super::queue::{TransactionFuture, TransactionQueue};
use super::transaction::{Key, Transaction, Value};
use crate::author::Author;
use crate::error::Error;
use crate::hash::FileHasher;
use async_std::prelude::*;
use core::ops::RangeBounds;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct Tree {
    tree: sled::Tree,
    queue: Arc<Mutex<TransactionQueue>>,
}

impl Tree {
    pub fn new(tree: sled::Tree, queue: Arc<Mutex<TransactionQueue>>) -> Self {
        Self { tree, queue }
    }

    pub fn get<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<Option<sled::IVec>> {
        self.tree.get(key)
    }

    pub fn watch_prefix<P: AsRef<[u8]>>(&self, prefix: P) -> sled::Subscriber {
        self.tree.watch_prefix(prefix)
    }

    pub fn contains_key<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<bool> {
        self.tree.contains_key(key)
    }

    pub fn get_lt<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<Option<(sled::IVec, sled::IVec)>> {
        self.tree.get_lt(key)
    }

    pub fn get_gt<K: AsRef<[u8]>>(&self, key: K) -> sled::Result<Option<(sled::IVec, sled::IVec)>> {
        self.tree.get_gt(key)
    }

    pub fn iter(&self) -> sled::Iter {
        self.tree.iter()
    }

    pub fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> sled::Iter {
        self.tree.range(range)
    }

    pub fn scan_prefix<P: AsRef<[u8]>>(&self, prefix: P) -> sled::Iter {
        self.tree.scan_prefix(prefix)
    }

    pub fn len(&self) -> usize {
        self.tree.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn name(&self) -> sled::IVec {
        self.tree.name()
    }

    pub fn checksum(&self) -> sled::Result<u32> {
        self.tree.checksum()
    }

    pub fn insert<P: AsRef<[u8]>, K: AsRef<[u8]>, V: Into<Value>>(
        &self,
        prefix: P,
        key: K,
        value: V,
    ) -> Result<TransactionFuture, Error> {
        let key = Key::new(prefix, key)?;
        let tx = Transaction::Insert(key, value.into());
        Ok(self.queue.lock().unwrap().create_transaction(tx)?)
    }

    pub fn remove<P: AsRef<[u8]>, K: AsRef<[u8]>>(
        &self,
        prefix: P,
        key: K,
    ) -> Result<TransactionFuture, Error> {
        let key = Key::new(prefix, key)?;
        let tx = Transaction::Remove(key);
        Ok(self.queue.lock().unwrap().create_transaction(tx)?)
    }

    pub fn compare_and_swap<P: AsRef<[u8]>, K: AsRef<[u8]>>(
        &self,
        prefix: P,
        key: K,
        old: Option<Value>,
        new: Option<Value>,
    ) -> Result<TransactionFuture, Error> {
        let key = Key::new(prefix, key)?;
        let tx = Transaction::CompareAndSwap(key, old, new);
        Ok(self.queue.lock().unwrap().create_transaction(tx)?)
    }

    pub fn add_author_to_prefix<P: Into<Value>>(
        &self,
        prefix: P,
        author: Author,
    ) -> Result<TransactionFuture, Error> {
        let tx = Transaction::AddAuthorToPrefix(prefix.into(), author);
        Ok(self.queue.lock().unwrap().create_transaction(tx)?)
    }

    pub fn remove_author_from_prefix<P: Into<Value>>(
        &self,
        prefix: P,
        author: Author,
    ) -> Result<TransactionFuture, Error> {
        let tx = Transaction::RemAuthorFromPrefix(prefix.into(), author);
        Ok(self.queue.lock().unwrap().create_transaction(tx)?)
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
