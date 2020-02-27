//! Author tracking.
use crate::error::Error;
use async_std::fs::{File, Permissions};
use async_std::path::Path;
use async_std::{fs, prelude::*};
use core::cmp::Ordering;
use core::hash::{Hash, Hasher};
use core::ops::Deref;
pub use disco::ed25519::Signature;
use disco::ed25519::{Keypair, PublicKey, SignatureError};
use rand::rngs::OsRng;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Author(PublicKey);

impl Deref for Author {
    type Target = PublicKey;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialOrd for Author {
    fn partial_cmp(&self, other: &Author) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Author {
    fn cmp(&self, other: &Author) -> Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

impl Hash for Author {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_bytes().hash(h);
    }
}

impl Author {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SignatureError> {
        Ok(Self(PublicKey::from_bytes(bytes)?))
    }
}

#[derive(Debug)]
pub struct Identity(Keypair);

impl Identity {
    pub fn generate() -> Self {
        Self(Keypair::generate(&mut OsRng))
    }

    pub fn sign(&self, msg: &[u8]) -> Signature {
        self.0.sign(msg)
    }

    pub fn author(&self) -> Author {
        Author(self.0.public)
    }

    pub async fn load_from(path: &Path) -> Result<Self, Error> {
        if !path.exists().await {
            let key = Self::generate();
            let bytes = key.0.to_bytes();
            let mut file = File::create(path).await?;
            #[cfg(unix)]
            file.set_permissions(Permissions::from_mode(0o600)).await?;
            file.write(&bytes[..]).await?;
        }
        let bytes = fs::read(path).await?;
        let key = Keypair::from_bytes(&bytes)?;
        Ok(Self(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempdir::TempDir;

    #[async_std::test]
    async fn load_from() {
        let tmp = TempDir::new("load_from").unwrap();
        let path = tmp.path().join("identity");
        let path: &Path = path.as_path().into();
        let key1 = Identity::load_from(path).await.unwrap();
        let key2 = Identity::load_from(path).await.unwrap();
        assert_eq!(&key1.0.to_bytes()[..], &key2.0.to_bytes()[..]);
    }
}
