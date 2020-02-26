use async_std::io;
use disco::ed25519::SignatureError;
use std::time::SystemTimeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Sig(#[from] SignatureError),
    #[error("Config directory was not found")]
    ConfigDir,
}

#[derive(Debug, Error)]
pub enum StateError {
    #[error("{0}")]
    Sled(#[from] sled::Error),
    #[error("{0}")]
    Sig(#[from] SignatureError),
    #[error("Invalid checkpoint")]
    InvalidCheckpoint,
    #[error("{0}")]
    Hash(#[from] HashError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Bincode(#[from] bincode::Error),
}

#[derive(Debug, Error)]
pub enum HashError {
    #[error("{0}")]
    Time(#[from] SystemTimeError),
    #[error("{0}")]
    Io(#[from] io::Error),
}
