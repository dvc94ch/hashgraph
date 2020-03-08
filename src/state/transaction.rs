use crate::author::{Author, Signature};
use crate::error::Error;
use serde::{de::Error as SerdeError, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Transaction {
    AddAuthor(Author, u64),
    RemAuthor(Author, u64),
    SignBlock(Signature),
    Insert(Key, Value),
    Remove(Key),
    AddAuthorToPrefix(Value, Author),
    RemAuthorFromPrefix(Value, Author),
    CompareAndSwap(Key, Option<Value>, Option<Value>),
    SignCheckpoint(Signature),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum TransactionError {
    Permission,
    CompareAndSwap {
        current: Option<Value>,
        proposed: Option<Value>,
    },
}

pub type TransactionResult = Result<(), TransactionError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Key(Box<[u8]>);

impl Key {
    pub fn new<P: AsRef<[u8]>, K: AsRef<[u8]>>(prefix: P, key: K) -> Result<Self, Error> {
        let prefix = prefix.as_ref();
        let key = key.as_ref();
        if prefix.len() > core::u8::MAX as usize {
            return Err(Error::InvalidKey);
        }
        let mut bytes = Vec::with_capacity(prefix.len() + key.len() + 1);
        bytes.push(prefix.len() as u8);
        bytes.extend_from_slice(prefix);
        bytes.extend_from_slice(key);
        Ok(Self(bytes.into_boxed_slice()))
    }

    pub fn prefix(&self) -> &[u8] {
        let end = self.0[0] as usize + 1;
        &self.0[1..end]
    }

    pub fn key(&self) -> &[u8] {
        let start = self.0[0] as usize + 1;
        &self.0[start..]
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes[0] as usize + 1 > bytes.len() {
            return Err(Error::InvalidKey);
        }
        Ok(Key(bytes.to_vec().into_boxed_slice()))
    }
}

impl AsRef<[u8]> for Key {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Serialize for Key {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.as_ref())
    }
}

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes: &[u8] = Deserialize::deserialize(deserializer)?;
        Self::from_bytes(bytes).map_err(SerdeError::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value(Box<[u8]>);

impl Value {
    pub fn new<V: AsRef<[u8]>>(value: V) -> Self {
        Self(value.as_ref().to_vec().into_boxed_slice())
    }
}

impl AsRef<[u8]> for Value {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.as_ref())
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let bytes: &[u8] = Deserialize::deserialize(deserializer)?;
        Ok(Self::new(bytes))
    }
}
