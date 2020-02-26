//! Readonly tree.
use core::ops::RangeBounds;

#[derive(Debug, Clone)]
pub struct Tree(pub(crate) sled::Tree);

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
