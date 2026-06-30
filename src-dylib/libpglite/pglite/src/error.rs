use std::error::Error as StdError;
use std::result::Result as StdResult;

use thiserror::Error;

/// Errors produced while installing, running, or querying PGlite.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{severity}: {message} (SQLSTATE {code})")]
    Database {
        severity: String,
        code: String,
        message: String,
    },
    #[error("{0}")]
    Runtime(String),
    #[error("{message}: {source}")]
    Context {
        message: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}

impl Error {
    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self::Runtime(message.into())
    }
}

pub type Result<T> = StdResult<T, Error>;

pub(crate) trait Context<T> {
    fn context(self, message: impl Into<String>) -> Result<T>;
    fn with_context(self, message: impl FnOnce() -> String) -> Result<T>;
}

impl<T, E> Context<T> for StdResult<T, E>
where
    E: StdError + Send + Sync + 'static,
{
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.map_err(|source| Error::Context {
            message: message.into(),
            source: Box::new(source),
        })
    }

    fn with_context(self, message: impl FnOnce() -> String) -> Result<T> {
        self.map_err(|source| Error::Context {
            message: message(),
            source: Box::new(source),
        })
    }
}

impl<T> Context<T> for Option<T> {
    fn context(self, message: impl Into<String>) -> Result<T> {
        self.ok_or_else(|| Error::Runtime(message.into()))
    }

    fn with_context(self, message: impl FnOnce() -> String) -> Result<T> {
        self.ok_or_else(|| Error::Runtime(message()))
    }
}

macro_rules! runtime_error {
    ($($arg:tt)*) => {
        crate::Error::message(format!($($arg)*))
    };
}

macro_rules! bail {
    ($($arg:tt)*) => {
        return Err(runtime_error!($($arg)*))
    };
}

macro_rules! ensure {
    ($condition:expr, $($arg:tt)*) => {
        if !$condition {
            bail!($($arg)*);
        }
    };
}
