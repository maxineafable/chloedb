use std::fmt;
use std::num::ParseIntError;
use std::str::Utf8Error;

use crate::binarylog::BinaryLogError;

#[derive(Debug)]
pub enum DBError {
    LogCountNotEnough,
    InvalidSliceLength(std::array::TryFromSliceError),
    KeyNotFound,
    CorruptedValue,
    FailedParseFileId,
    FailedByteKeyConvert,
    KeyAlreadyExists,
    ValueTooLarge,
    IoError(std::io::Error),
}

impl fmt::Display for DBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DBError::LogCountNotEnough => write!(f, "Log file count not enough for compaction"),
            DBError::InvalidSliceLength(err) => {
                write!(f, "Invalid slice length to convert: {}", err)
            }
            DBError::KeyNotFound => write!(f, "Key not found"),
            DBError::CorruptedValue => write!(f, "Corrupted value"),
            DBError::FailedParseFileId => write!(f, "Failed parsing file into id"),
            DBError::FailedByteKeyConvert => write!(f, "Failed converting key bytes into String"),
            DBError::KeyAlreadyExists => write!(f, "Key already exists"),
            DBError::ValueTooLarge => write!(f, "First value exceeds the byte limit"),
            DBError::IoError(io_err) => write!(f, "IO Error: {}", io_err),
        }
    }
}

impl From<std::io::Error> for DBError {
    fn from(value: std::io::Error) -> Self {
        DBError::IoError(value)
    }
}

impl From<std::array::TryFromSliceError> for DBError {
    fn from(err: std::array::TryFromSliceError) -> Self {
        DBError::InvalidSliceLength(err)
    }
}

impl From<BinaryLogError> for DBError {
    fn from(_: BinaryLogError) -> Self {
        DBError::CorruptedValue
    }
}

impl From<ParseIntError> for DBError {
    fn from(_: ParseIntError) -> Self {
        DBError::FailedParseFileId
    }
}

impl From<Utf8Error> for DBError {
    fn from(_: Utf8Error) -> Self {
        DBError::FailedByteKeyConvert
    }
}
