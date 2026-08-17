use std::fmt;
use std::fs;
use std::io;
use std::io::BufWriter;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::num::ParseIntError;
use std::path::Path;
use std::path::PathBuf;
use std::str::Utf8Error;
use std::sync::{Arc, RwLock};
use std::thread;
use std::{collections::HashMap, fs::File, io::BufReader};

use crate::binarylog;
use crate::binarylog::OperationTypeError;
use crate::file;

#[derive(Debug, Clone)]
struct MapValue {
    offset: u32,
    record_size: u32,
    file_id: u32,
}

pub struct DB {
    map: Arc<RwLock<HashMap<String, MapValue>>>,
    active_file: PathBuf,
    active_file_id: u32,
    byte_counter: u32,
    dir: PathBuf,
}

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

impl DB {
    pub fn open() -> Result<Self, DBError> {
        let dir = Path::new("logs");

        if !dir.exists() {
            std::fs::create_dir(dir)?;
        }

        let map = Arc::new(RwLock::new(HashMap::new()));
        let mut byte_counter = 0;

        let map_clone = Arc::clone(&map);
        let mut map_guard = map_clone.write().unwrap();

        let mut file_id = 1;
        let mut active_file = file::format_log_file_path(dir, file_id);

        // Need to sort them to read log file sequentially
        let entries = fs::read_dir(dir)?;
        let mut paths: Vec<_> = entries.filter_map(|r| r.ok()).collect();
        paths.sort_by_key(|dir| dir.path());

        for read in paths {
            let path = read.path();

            let mut file_offset: u32 = 0;

            if !path.is_file() {
                continue;
            }

            active_file = PathBuf::from(&path);
            byte_counter = fs::metadata(&path)?.len();
            file_id = path.file_stem().unwrap().to_str().unwrap().parse()?;

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

                let log = binarylog::BinaryLog::deserialize(&record)?;

                match log.get_op_type() {
                    binarylog::OperationType::Set => map_guard.insert(
                        str::from_utf8(log.get_key()).unwrap().to_string(),
                        MapValue {
                            offset: file_offset,
                            record_size: record_len,
                            file_id,
                        },
                    ),
                    binarylog::OperationType::Remove => {
                        map_guard.remove(str::from_utf8(log.get_key())?)
                    }
                };

                let total_len = record_len + std::mem::size_of::<u32>() as u32;

                file_offset += total_len;
            }
        }

        Ok(Self {
            map,
            active_file,
            active_file_id: file_id,
            byte_counter: byte_counter as u32,
            dir: dir.to_path_buf(),
        })
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), DBError> {
        let map_clone = Arc::clone(&self.map);

        let has_key = {
            let map_guard = map_clone.read().unwrap();
            map_guard.contains_key(&key)
        };

        if has_key {
            return Err(DBError::KeyAlreadyExists);
        }

        let serialized = binarylog::BinaryLog::set(&key, &value);
        let record_len = serialized[..4].try_into()?;

        let num = u32::from_le_bytes(record_len);

        let total_len = num + std::mem::size_of::<u32>() as u32;

        let append_log_total = self.byte_counter + total_len;

        self.append_log(&serialized, append_log_total)?;

        let val = MapValue {
            offset: self.byte_counter,
            record_size: num,
            file_id: self.active_file_id,
        };

        let mut map_guard = map_clone.write().unwrap();
        map_guard.insert(key, val);

        Ok(())
    }

    pub fn get(&mut self, key: &str) -> Result<binarylog::BinaryLog, DBError> {
        let map_clone = Arc::clone(&self.map);
        let map_guard = map_clone.read().unwrap();

        if let Some(value) = map_guard.get(key) {
            let cur_path = file::format_log_file_path(&self.dir, value.file_id);

            let file = File::open(cur_path)?;
            let mut reader = BufReader::new(file);

            reader.seek(io::SeekFrom::Start(
                value.offset as u64 + std::mem::size_of::<u32>() as u64,
            ))?;

            let mut record = vec![0u8; value.record_size as usize];

            reader.read_exact(&mut record)?;

            let log = binarylog::BinaryLog::deserialize(&record)?;
            Ok(log)
        } else {
            // return Err("Key does not exist.".into());
            Err(DBError::KeyNotFound(key.to_string()))
        }
    }

    pub fn remove(&mut self, key: String) -> Result<(), DBError> {
        let map_clone = Arc::clone(&self.map);
        let mut map_guard = map_clone.write().unwrap();

        let serialized = binarylog::BinaryLog::remove(&key);
        let len_buf: [u8; 4] = serialized[0..4].try_into()?;

        let record_len = u32::from_le_bytes(len_buf);

        let total_len = record_len + std::mem::size_of::<u32>() as u32;

        let append_log_total = self.byte_counter + total_len;

        self.append_log(&serialized, append_log_total)?;

        map_guard.remove(&key);

        Ok(())
    }

    pub fn compact_log(&mut self) -> Result<(), DBError> {
        let log_file_count =
            file::get_log_file_count(None).map_err(|_| DBError::LogCountNotEnough)?;

        // 3 files only to test log compaction
        // after compaction, you'll have 2 log files:
        // the active file and the temporary that's renamed
        if log_file_count <= 3 {
            return Ok(());
        }

        let tmp_file = "temp.log";
        let tmp_path = self.dir.join(tmp_file);

        let map_clone = Arc::clone(&self.map);

        let dir_clone = self.dir.clone();
        let tmp_path_clone = tmp_path.clone();
        let active_file_id_clone = self.active_file_id.clone();

        let handle = thread::spawn(move || -> Result<(), DBError> {
            let map_entries: Vec<(String, MapValue)> = {
                let map_guard = map_clone.read().unwrap();
                map_guard
                    .iter()
                    // don't compact values currently in active file
                    .filter(|(_, v)| v.file_id != active_file_id_clone)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            };

            let tmp_log = File::create(tmp_path_clone)?;
            let mut tmp_writer = BufWriter::new(tmp_log);

            for (_, val) in map_entries {
                let cur_path = file::format_log_file_path(&dir_clone, val.file_id);

                let file = File::open(cur_path)?;
                let mut reader = BufReader::new(file);

                reader.seek(io::SeekFrom::Start(val.offset as u64))?;

                let mut record =
                    vec![0u8; val.record_size as usize + std::mem::size_of::<u32>() as usize];

                reader.read_exact(&mut record)?;

                tmp_writer.write_all(&record)?;
            }

            tmp_writer.flush()?;
            tmp_writer.get_ref().sync_all()?;

            Ok(())
        });

        handle.join().unwrap()?;

        let dest_tmp_log = self.active_file_id - 1;
        let cur_path = file::format_log_file_path(&self.dir, dest_tmp_log);

        fs::rename(tmp_path, &cur_path)?;

        file::remove_prev_log_files(&self.dir)?;
        file::set_prevlog_readonly(cur_path)?;

        Ok(())
    }

    pub fn list(&self) -> Vec<String> {
        let map_clone = Arc::clone(&self.map);
        let map_guard = map_clone.read().unwrap();

        map_guard.keys().cloned().collect()
    }

    fn append_log(&mut self, serialized: &[u8], append_log_total: u32) -> Result<(), DBError> {
        if file::check_log_threshold(append_log_total) {
            file::set_prevlog_readonly(&self.active_file)?;
            self.set_new_active_file();
        }

        let file = file::open_log(&self.active_file, false)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(serialized)?;
        writer.flush()?;

        Ok(())
    }

    fn set_new_active_file(&mut self) {
        self.active_file_id += 1;

        self.active_file = file::format_log_file_path(&self.dir, self.active_file_id);
        self.byte_counter = 0;
    }
}
