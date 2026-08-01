use chrono::Utc;
use core::fmt;
use crc32fast::Hasher;
use std::fs;
use std::io;
use std::io::BufWriter;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::{collections::HashMap, fs::File, io::BufReader};

use crate::file::open_log;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum OperationType {
    Set = 1,
    Remove = 2,
}

impl TryFrom<u8> for OperationType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(OperationType::Set),
            2 => Ok(OperationType::Remove),
            _ => Err(()),
        }
    }
}

pub struct BinaryLog {
    crc: u32,
    timestamp: i64,
    operation_type: OperationType,
    key: Vec<u8>,
    val: Vec<u8>,
}

impl fmt::Display for BinaryLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(&self.val) {
            Ok(s) => write!(f, "Value: {}", s),
            Err(e) => write!(f, "Value: <invalid UTF-8: {}>", e),
        }
    }
}

impl BinaryLog {
    fn set(key: &str, value: &str) -> Self {
        Self {
            crc: 0,
            timestamp: Utc::now().timestamp(),
            operation_type: OperationType::Set,
            key: key.as_bytes().to_vec(),
            val: value.as_bytes().to_vec(),
        }
    }

    fn remove(key: &str) -> Self {
        Self {
            crc: 0,
            timestamp: Utc::now().timestamp(),
            operation_type: OperationType::Remove,
            key: key.as_bytes().to_vec(),
            val: Vec::new(),
        }
    }
}

struct MapValue {
    offset: u32,
    record_size: u32,
}

pub struct DB {
    map: HashMap<String, MapValue>,
    log_file: PathBuf,
    tmp_log_file: PathBuf,
    byte_counter: u32,
}

impl DB {
    const TEN_MB: u64 = 10 * 1024 * 1024;

    pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();

        let mut map = HashMap::new();
        let mut byte_counter: u32 = 0;

        if path.exists() {
            let file = File::open(&path)?;
            let mut reader = BufReader::new(file);

            loop {
                let mut len_buf = [0u8; 4];

                match reader.read_exact(&mut len_buf) {
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                    Err(e) => {
                        println!("Another I/O error occurred: {}", e);
                    }
                }

                let record_len = u32::from_le_bytes(len_buf);

                let mut record = vec![0u8; record_len as usize];

                reader.read_exact(&mut record)?;

                let log = DB::deserialize(&record)?;

                match log.operation_type {
                    OperationType::Set => map.insert(
                        String::from_utf8(log.key)?,
                        MapValue {
                            offset: byte_counter,
                            record_size: record_len,
                        },
                    ),
                    OperationType::Remove => map.remove(&String::from_utf8(log.key)?),
                };

                let total_len = record_len + std::mem::size_of::<u32>() as u32;

                byte_counter += total_len;
            }
        }

        Ok(Self {
            map,
            log_file: path.to_path_buf(),
            tmp_log_file: path.with_added_extension("tmp"),
            byte_counter,
        })
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), Box<dyn std::error::Error>> {
        let log = BinaryLog::set(&key, &value);
        let serialized = self.serialize(&log);

        let record_len = serialized[..4].try_into()?;

        let num = u32::from_le_bytes(record_len);

        self.append_log(&serialized)?;

        self.map.insert(
            key,
            MapValue {
                offset: self.byte_counter,
                record_size: num,
            },
        );

        let total_len = num + std::mem::size_of::<u32>() as u32;

        self.byte_counter += total_len;

        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<BinaryLog, Box<dyn std::error::Error>> {
        let file = File::open(&self.log_file)?;
        let mut reader = BufReader::new(file);

        if let Some(value) = self.map.get(key) {
            reader.seek(io::SeekFrom::Start(
                value.offset as u64 + std::mem::size_of::<u32>() as u64,
            ))?;

            let mut record = vec![0u8; value.record_size as usize];

            reader.read_exact(&mut record)?;

            let log = DB::deserialize(&record)?;
            Ok(log)
        } else {
            return Err("Key does not exist.".into());
        }
    }

    pub fn remove(&mut self, key: String) -> Result<(), Box<dyn std::error::Error>> {
        let log = BinaryLog::remove(&key);
        let serialized = self.serialize(&log);

        self.append_log(&serialized)?;

        self.map.remove(&key);

        Ok(())
    }

    pub fn compact_log(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.log_file.exists() {
            let metadata = fs::metadata(&self.log_file)?;
            if metadata.len() > DB::TEN_MB {
                {
                    let tmp_log = open_log(&self.tmp_log_file, true)?;
                    let mut tmp_writer = BufWriter::new(tmp_log);
                    for (_, val) in self.map.iter() {
                        let file = File::open(&self.log_file)?;
                        let mut reader = BufReader::new(file);

                        reader.seek(io::SeekFrom::Start(
                            val.offset as u64 + std::mem::size_of::<u32>() as u64,
                        ))?;

                        let mut record = vec![0u8; val.record_size as usize];

                        reader.read_exact(&mut record)?;

                        tmp_writer.write_all(&record)?;
                    }
                    tmp_writer.flush()?;
                }
                fs::rename(&self.tmp_log_file, &self.log_file)?;
            }
        }

        Ok(())
    }

    fn append_log(&self, serialized: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let file = open_log(&self.log_file, false)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(serialized)?;
        writer.flush()?;

        Ok(())
    }

    fn deserialize(record: &[u8]) -> Result<BinaryLog, Box<dyn std::error::Error>> {
        let mut pos = 0;

        let stored_crc = u32::from_le_bytes(record[pos..pos + 4].try_into()?);
        let computed_crc = DB::compute_crc(&record);

        if computed_crc != stored_crc {
            return Err("CRC mismatch".into());
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
            operation_type: OperationType::try_from(operation)
                .map_err(|_| "invalid operation type")?,
            timestamp,
            val: value,
        })
    }

    fn serialize(&self, log: &BinaryLog) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&log.crc.to_le_bytes());
        bytes.extend_from_slice(&log.timestamp.to_le_bytes());
        bytes.push(log.operation_type as u8);
        bytes.extend_from_slice(&(log.key.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(log.val.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&log.key);
        bytes.extend_from_slice(&log.val);

        let crc = DB::compute_crc(&bytes);

        bytes[0..4].copy_from_slice(&crc.to_le_bytes());

        let mut out = Vec::new();
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend(bytes);

        out
    }

    fn compute_crc(bytes: &[u8]) -> u32 {
        let mut hasher = Hasher::new();
        hasher.update(&bytes[4..]);
        hasher.finalize()
    }
}
