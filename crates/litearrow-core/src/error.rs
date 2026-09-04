use std::fmt;

#[derive(Debug)]
pub enum Error {
    UnexpectedEndOfInput,
    InvalidMagic { structure: &'static str },
    InvalidUtf8,
    InvalidMetadata(&'static str),
    ChecksumMismatch { structure: &'static str },
    Arrow(String),
    Io(String),
    IntegerOverflow,
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

impl From<arrow_schema::ArrowError> for Error {
    fn from(error: arrow_schema::ArrowError) -> Self {
        Self::Arrow(error.to_string())
    }
}
