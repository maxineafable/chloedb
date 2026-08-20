use chrono::Utc;
use core::fmt;
use crc32fast::Hasher;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum OperationType {
    Set = 1,
    Remove = 2,
}

#[derive(Debug)]
pub enum BinaryLogError {
    InvalidOperationType,
    InvalidCRC,
    InvalidSliceLength(std::array::TryFromSliceError),
}

impl fmt::Display for BinaryLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinaryLogError::InvalidOperationType => write!(f, "Invalid operation type"),
            BinaryLogError::InvalidCRC => write!(f, "Invalid CRC"),
            BinaryLogError::InvalidSliceLength(err) => {
                write!(f, "Invalid slice length to convert: {}", err)
            },
        }
    }
}

impl TryFrom<u8> for OperationType {
    type Error = BinaryLogError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(OperationType::Set),
            2 => Ok(OperationType::Remove),
            _ => Err(BinaryLogError::InvalidOperationType),
        }
    }
}

impl From<std::array::TryFromSliceError> for BinaryLogError {
    fn from(err: std::array::TryFromSliceError) -> Self {
        BinaryLogError::InvalidSliceLength(err)
    }
}
pub struct BinaryLog {
    crc: u32,
    timestamp: i64,
    operation_type: OperationType,
    key: Vec<u8>,
    val: Vec<u8>,
}

impl BinaryLog {
    pub fn set(key: &[u8], value: &[u8]) -> Vec<u8> {
        BinaryLog::serialize(BinaryLog {
            crc: 0,
            timestamp: Utc::now().timestamp(),
            operation_type: OperationType::Set,
            key: key.to_vec(),
            val: value.to_vec(),
        })
    }

    pub fn remove(key: &[u8]) -> Vec<u8> {
        BinaryLog::serialize(BinaryLog {
            crc: 0,
            timestamp: Utc::now().timestamp(),
            operation_type: OperationType::Remove,
            key: key.to_vec(),
            val: Vec::new(),
        })
    }

    fn serialize(log: BinaryLog) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&log.crc.to_le_bytes());
        bytes.extend_from_slice(&log.timestamp.to_le_bytes());
        bytes.push(log.operation_type as u8);
        bytes.extend_from_slice(&(log.key.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(log.val.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&log.key);
        bytes.extend_from_slice(&log.val);

        let crc = BinaryLog::compute_crc(&bytes);

        bytes[0..4].copy_from_slice(&crc.to_le_bytes());

        let mut out = Vec::new();
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend(bytes);

        out
    }

    pub fn deserialize(record: &[u8]) -> Result<BinaryLog, BinaryLogError> {
        let mut pos = 0;

        let stored_crc = u32::from_le_bytes(record[pos..pos + 4].try_into()?);
        let computed_crc = BinaryLog::compute_crc(&record);

        if computed_crc != stored_crc {
            return Err(BinaryLogError::InvalidCRC);
        }

        pos += 4;

        let timestamp = i64::from_le_bytes(record[pos..pos + 8].try_into()?);
        pos += 8;

        let operation = record[pos];
        pos += 1;

        let key_len = u32::from_le_bytes(record[pos..pos + 4].try_into()?) as usize;
        pos += 4;

        let val_len = u32::from_le_bytes(record[pos..pos + 4].try_into()?) as usize;
        pos += 4;

        let key = record[pos..pos + key_len].to_vec();
        pos += key_len;

        let value = record[pos..pos + val_len].to_vec();

        Ok(BinaryLog {
            crc: computed_crc,
            key,
            operation_type: OperationType::try_from(operation)?,
            // operation_type: OperationType::try_from(operation)
            // .map_err(|_| "invalid operation type")?,
            timestamp,
            val: value,
        })
    }

    fn compute_crc(bytes: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(&bytes[4..]);
        hasher.finalize()
    }

    pub fn get_op_type(&self) -> OperationType {
        self.operation_type
    }

    pub fn get_key(self) -> Vec<u8> {
        self.key
    }
}

impl fmt::Display for BinaryLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(&self.val) {
            Ok(s) => write!(f, "{}", s),
            Err(e) => write!(f, "Error: {}", e),
        }
    }
}
