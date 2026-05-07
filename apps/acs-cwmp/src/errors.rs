use std::str::Utf8Error;

use thiserror::Error;

#[allow(clippy::module_name_repetitions)]
#[derive(Error, Debug)]
pub enum AcsError {
    #[error("Invalid auth response towards Airties")]
    InvalidAuthResponse,
    #[error("UTF-8 decode error {0}")]
    UTF8DecodeError(#[from] Utf8Error),
    #[error("Reqwest Error {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Serde Error {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("Env var read Error {0}")]
    Var(#[from] std::env::VarError),
    #[error("No payload in borrowed message")]
    NoMessagePayload,
    #[error("No inform payload in borrowed message")]
    NoInformPayload,
    #[error("No key in borrowed message")]
    NoMessageKey,
    #[error("The Airties BearerToken refresh part looks invalid")]
    InvalidRefreshToken,
    #[error("The model in the message is not a mesh node")]
    InvalidModel,
    #[error("Serial number does not look valid")]
    InvalidSerialNumber,
}
