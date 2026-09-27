use rusqlite::Error as SqliteError;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Msg(String),
    #[error("confirmation required for this privileged action")]
    ConfirmationRequired,
    #[error(transparent)]
    Sqlite(#[from] SqliteError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("invalid argv: {0}")]
    Argv(String),
}

impl Error {
    pub fn msg(m: impl Into<String>) -> Self {
        Self::Msg(m.into())
    }
}

impl serde::Serialize for Error {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
