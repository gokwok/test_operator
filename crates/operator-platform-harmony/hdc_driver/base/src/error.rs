use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

use openssl::error::ErrorStack;
use serde_json::Error as JsonError;

pub type Result<T> = std::result::Result<T, HdcError>;

#[derive(Debug)]
pub enum HdcError {
    Io(std::io::Error),
    OpenSsl(ErrorStack),
    Utf8(std::string::FromUtf8Error),
    ParseInt(std::num::ParseIntError),
    Json(JsonError),
    Protocol(String),
    Cli(String),
}

impl HdcError {
    pub fn protocol(message: impl Into<String>) -> Self {
        Self::Protocol(message.into())
    }

    pub fn cli(message: impl Into<String>) -> Self {
        Self::Cli(message.into())
    }
}

impl Display for HdcError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::OpenSsl(error) => write!(f, "{error}"),
            Self::Utf8(error) => write!(f, "{error}"),
            Self::ParseInt(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::Protocol(message) => write!(f, "{message}"),
            Self::Cli(message) => write!(f, "{message}"),
        }
    }
}

impl StdError for HdcError {}

impl From<std::io::Error> for HdcError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ErrorStack> for HdcError {
    fn from(value: ErrorStack) -> Self {
        Self::OpenSsl(value)
    }
}

impl From<std::string::FromUtf8Error> for HdcError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        Self::Utf8(value)
    }
}

impl From<std::num::ParseIntError> for HdcError {
    fn from(value: std::num::ParseIntError) -> Self {
        Self::ParseInt(value)
    }
}

impl From<JsonError> for HdcError {
    fn from(value: JsonError) -> Self {
        Self::Json(value)
    }
}
