use crate::error::Error;
use async_std::fs::{self, File};
use async_std::io::{Read, Result as IoResult, Write};
use async_std::path::{Path, PathBuf};
use async_std::task::{Context, Poll};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use data_encoding::BASE32;
use disco::symmetric::DiscoHash;
use std::time::{SystemTime, UNIX_EPOCH};

pub const HASH_LENGTH: usize = 32;
pub const GENESIS_HASH: Hash = Hash([0u8; 32]);

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct Hash([u8; HASH_LENGTH]);

impl core::fmt::Debug for Hash {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", BASE32.encode(&self.0))
    }
}

impl Deref for Hash {
    type Target = [u8; HASH_LENGTH];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Hash {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut hash = [0u8; HASH_LENGTH];
        hash.clone_from_slice(&bytes);
        Self(hash)
    }

    #[cfg(test)]
    pub fn random() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        Self(rng.gen())
    }
}

pub struct Hasher {
    hasher: DiscoHash,
}

impl Hasher {
    pub fn new() -> Self {
        Self {
            hasher: DiscoHash::new(HASH_LENGTH),
        }
    }

    pub fn sum(self) -> Hash {
        let bytes = self.hasher.sum();
        Hash::from_bytes(&bytes)
    }
}

impl Deref for Hasher {
    type Target = DiscoHash;

    fn deref(&self) -> &Self::Target {
        &self.hasher
    }
}

impl DerefMut for Hasher {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hasher
    }
}

pub struct FileHasher {
    hasher: Hasher,
    file: File,
    path: PathBuf,
}

impl FileHasher {
    pub fn path_for_hash(dir: &Path, hash: &Hash) -> PathBuf {
        dir.join(BASE32.encode(&**hash))
    }

    pub async fn create_tmp(dir: &Path) -> Result<Self, Error> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let mut hasher = Hasher::new();
        hasher.write(&timestamp.to_be_bytes());
        let path = Self::path_for_hash(dir, &hasher.sum());
        Self::create(path.as_ref()).await
    }

    pub async fn create(path: &Path) -> Result<Self, Error> {
        let file = File::create(path).await?;
        let hasher = Hasher::new();
        Ok(Self {
            hasher,
            file,
            path: path.to_path_buf(),
        })
    }

    pub async fn open_with_hash(dir: &Path, hash: &Hash) -> Result<Self, Error> {
        let path = Self::path_for_hash(dir, hash);
        Self::open(&path).await
    }

    pub async fn open(path: &Path) -> Result<Self, Error> {
        let file = File::open(path).await?;
        let hasher = Hasher::new();
        Ok(Self {
            hasher,
            file,
            path: path.to_path_buf(),
        })
    }

    pub async fn rename(self, dir: &Path) -> Result<Hash, Error> {
        let hash = self.hasher.sum();
        let path = Self::path_for_hash(dir, &hash);
        fs::rename(self.path, path).await?;
        Ok(hash)
    }

    pub fn hash(self) -> Hash {
        self.hasher.sum()
    }
}

impl Write for FileHasher {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<IoResult<usize>> {
        let poll = Pin::new(&mut self.file).poll_write(cx, buf);
        if let Poll::Ready(Ok(len)) = &poll {
            self.hasher.write(&buf[..*len]);
        }
        poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<IoResult<()>> {
        Pin::new(&mut self.file).poll_flush(cx)
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<IoResult<()>> {
        Pin::new(&mut self.file).poll_close(cx)
    }
}

impl Read for FileHasher {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<IoResult<usize>> {
        let poll = Pin::new(&mut self.file).poll_read(cx, buf);
        if let Poll::Ready(Ok(len)) = &poll {
            self.hasher.write(&buf[..*len]);
        }
        poll
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::prelude::*;
    use tempdir::TempDir;

    type Result = std::result::Result<(), Box<dyn std::error::Error>>;

    #[async_std::test]
    async fn test_file_hasher() -> Result {
        let data = b"hello world";
        let tmp = TempDir::new("test_file_hasher").unwrap();
        let tmp_path = tmp.path().join("new");
        let mut fh = FileHasher::create(tmp_path.as_path().into()).await?;
        let mut hasher = Hasher::new();
        fh.write(data).await?;
        hasher.write(data);
        let hash = fh.rename(tmp.path().into()).await?;
        assert_eq!(hasher.sum(), hash);
        Ok(())
    }

    #[async_std::test]
    async fn test_tmpfile_hasher() -> Result {
        let data = b"hello world";
        let tmp = TempDir::new("test_tmpfile_hasher").unwrap();
        let mut fh = FileHasher::create_tmp(tmp.path().into()).await?;
        let mut hasher = Hasher::new();
        fh.write(data).await?;
        hasher.write(data);
        let hash = fh.rename(tmp.path().into()).await?;
        assert_eq!(hasher.sum(), hash);
        Ok(())
    }

    #[async_std::test]
    async fn test_roundtrip() -> Result {
        let data = b"hello world";
        let tmp = TempDir::new("test_roundtrip").unwrap();
        let dir: &Path = tmp.path().into();
        let mut fh = FileHasher::create_tmp(dir).await?;
        fh.write(data).await?;
        let hash = fh.rename(dir).await?;
        let mut fh = FileHasher::open_with_hash(dir, &hash).await?;
        let mut buf = Vec::new();
        fh.read_to_end(&mut buf).await?;
        assert_eq!(fh.hash(), hash);
        Ok(())
    }
}
