//! Author tracking.
use crate::error::Error;
use async_std::fs::{File, Permissions};
use async_std::path::Path;
use async_std::{fs, prelude::*};
use core::cmp::Ordering;
use core::fmt::{Debug, Formatter, Result as FmtResult};
use core::hash::{Hash, Hasher};
use core::ops::Deref;
use data_encoding::BASE32;
use disco::ed25519::{Keypair, PublicKey, Signature as RawSignature, SignatureError};
use rand::rngs::OsRng;
use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Author(PublicKey);

impl Debug for Author {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", BASE32.encode(self.as_bytes()))
    }
}

impl Serialize for Author {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.as_bytes())
    }
}

impl<'de> Deserialize<'de> for Author {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes: &[u8] = Deserialize::deserialize(deserializer)?;
        Self::from_bytes(bytes).map_err(SerdeError::custom)
    }
}

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

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Signature(RawSignature);

impl Debug for Signature {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", BASE32.encode(&self.to_bytes()))
    }
}

impl Serialize for Signature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.to_bytes())
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes: &[u8] = Deserialize::deserialize(deserializer)?;
        Self::from_bytes(bytes).map_err(SerdeError::custom)
    }
}

impl Deref for Signature {
    type Target = RawSignature;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialOrd for Signature {
    fn partial_cmp(&self, other: &Signature) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Signature {
    fn cmp(&self, other: &Signature) -> Ordering {
        self.to_bytes().cmp(&other.to_bytes())
    }
}

impl Hash for Signature {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.to_bytes().hash(h);
    }
}

impl Signature {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SignatureError> {
        Ok(Self(RawSignature::from_bytes(bytes)?))
    }
}

#[derive(Debug)]
pub struct Identity(Keypair);

impl Identity {
    pub fn generate() -> Self {
        Self(Keypair::generate(&mut OsRng))
    }

    pub fn sign(&self, msg: &[u8]) -> Signature {
        Signature(self.0.sign(msg))
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
