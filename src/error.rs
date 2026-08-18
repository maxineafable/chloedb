use std::fmt;
use std::num::ParseIntError;
use std::str::Utf8Error;
use crate::binarylog::OperationTypeError;

#[derive(Debug)]
pub enum DBError {
    NotLogTmp,
    LogCountNotEnough,
    InvalidSliceLength(std::array::TryFromSliceError),
    KeyNotFound(String),
    InvalidCRC,
    InvalidOperationErr(OperationTypeError),
    FailedParseFileId,
    FailedByteKeyConvert,
    KeyAlreadyExists,
    IoError(std::io::Error),
}

impl fmt::Display for DBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DBError::NotLogTmp => write!(f, "Failed removing non temporary log"),
            DBError::LogCountNotEnough => write!(f, "Log file count not enough for compaction"),
            DBError::InvalidSliceLength(err) => {
                write!(f, "Invalid slice length to convert: {}", err)
            }
            DBError::KeyNotFound(key) => write!(f, "Key not found: {}", key),
            DBError::InvalidCRC => write!(f, "Invalid CRC"),
            DBError::InvalidOperationErr(err) => write!(f, "{}", err),
            DBError::FailedParseFileId => write!(f, "Failed parsing file into id"),
            DBError::FailedByteKeyConvert => write!(f, "Failed converting key bytes into String"),
            DBError::KeyAlreadyExists => write!(f, "Key already exists"),
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

impl From<OperationTypeError> for DBError {
    fn from(error: OperationTypeError) -> Self {
        DBError::InvalidOperationErr(error)
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
