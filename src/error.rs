use std::fmt;

#[derive(Debug)]
pub struct Error {
    pub code: u8,
    pub message: String,
}

impl Error {
    pub fn parameter(message: impl Into<String>) -> Self {
        Self {
            code: 252,
            message: message.into(),
        }
    }

    pub fn target(message: impl Into<String>) -> Self {
        Self {
            code: 254,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: 255,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::internal(value.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::internal(value.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
