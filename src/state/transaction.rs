use super::key::{Key, Value};
use crate::author::{Author, Signature};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum TransactionStatus {
    New(Transaction),
    Created,
    Commited(TransactionResult),
}
