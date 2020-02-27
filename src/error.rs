use async_std::io;
use disco::ed25519::SignatureError;
use std::time::SystemTimeError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid checkpoint")]
    InvalidCheckpoint,
    #[error("Invalid state")]
    InvalidState,
    #[error("Invalid block")]
    InvalidBlock,
    #[error("Invalid event")]
    InvalidEvent,

    #[error("Config directory was not found")]
    ConfigDir,
    #[error("{0}")]
    Sled(#[from] sled::Error),
    #[error("{0}")]
    Sig(#[from] SignatureError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Time(#[from] SystemTimeError),
    #[error("{0}")]
    Serde(#[from] bincode::Error),
}
