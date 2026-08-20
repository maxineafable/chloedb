use std::fs;
use std::io;
use std::io::BufWriter;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::{collections::HashMap, fs::File, io::BufReader};

use crate::binarylog;
use crate::file;
use crate::error::DBError;

#[derive(Debug, Clone)]
struct MapValue {
    offset: u32,
    record_size: u32,
    file_id: u32,
}

pub struct DB {
    map: HashMap<Vec<u8>, MapValue>,
    active_file: PathBuf,
    active_file_id: u32,
    byte_counter: u32,
    dir: PathBuf,
    max_bytes: u32,
    max_logs: u32,
}

impl DB {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, DBError> {
        let dir = dir.as_ref();

        if !dir.exists() {
            std::fs::create_dir(dir)?;
        }

        let mut map: HashMap<Vec<u8>, MapValue> = HashMap::new();

        let mut byte_counter: u32 = 0;

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

            active_file = path.to_path_buf();
            // TODO: temporary fix, handle the error from convert
            // but for now it is safe if using the default 1 MB max log file
            byte_counter = fs::metadata(&path)?.len().try_into().unwrap();

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
                    binarylog::OperationType::Set => map.insert(
                        log.get_key(),
                        MapValue {
                            offset: file_offset,
                            record_size: record_len,
                            file_id,
                        },
                    ),
                    binarylog::OperationType::Remove => {
                        map.remove(&log.get_key())
                    }
                };

                let total_len = record_len + std::mem::size_of::<u32>() as u32;

                file_offset += total_len;
            }
        }

        Ok(DB {
           map,
           active_file,
           active_file_id: file_id,
           byte_counter,
           dir: dir.to_path_buf(),
           max_bytes: 1024 * 1024, // 1 MB Default
           max_logs: 5, // To trigger log compaction
        })
    }

    pub fn set(&mut self, key: &[u8], value: &[u8]) -> Result<(), DBError> {
        if self.map.contains_key(key) {
            return Err(DBError::KeyAlreadyExists);
        }

        let serialized = binarylog::BinaryLog::set(key, value);
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

        self.map.insert(key.to_vec(), val);
        self.byte_counter += append_log_total;

        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<binarylog::BinaryLog, DBError> {
        if let Some(value) = self.map.get(key) {
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
            Err(DBError::KeyNotFound)
        }
    }

    pub fn remove(&mut self, key: &[u8]) -> Result<(), DBError> {
        if !self.map.contains_key(key) {
            return Err(DBError::KeyNotFound);
        }

        let serialized = binarylog::BinaryLog::remove(key);
        let len_buf: [u8; 4] = serialized[0..4].try_into()?;

        let record_len = u32::from_le_bytes(len_buf);

        let total_len = record_len + std::mem::size_of::<u32>() as u32;

        let append_log_total = self.byte_counter + total_len;

        self.append_log(&serialized, append_log_total)?;

        self.map.remove(key);
        self.byte_counter += append_log_total;

        Ok(())
    }

    fn compact_log(&self) -> Result<(), DBError> {
        let tmp_file = "temp.log";
        let tmp_path = self.dir.join(tmp_file);

        let tmp_log = File::create(&tmp_path)?;
        let mut tmp_writer = BufWriter::new(tmp_log);

        for (_, val) in self.map.iter() {
            if val.file_id == self.active_file_id {
                continue;
            }

            let cur_path = file::format_log_file_path(&self.dir, val.file_id);

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

        let dest_tmp_log = self.active_file_id - 1;
        let cur_path = file::format_log_file_path(&self.dir, dest_tmp_log);

        fs::rename(tmp_path, &cur_path)?;

        file::remove_prev_log_files(&self.dir)?;
        file::set_prevlog_readonly(cur_path)?;

        Ok(())
    }

    pub fn list(&self) -> Vec<Vec<u8>> {
        self.map.keys().cloned().collect()
    }

    fn append_log(&mut self, serialized: &[u8], append_log_total: u32) -> Result<(), DBError> {
        let exceeded = file::log_threshold_exceed(self.max_bytes, append_log_total);
        if self.active_file_id == 1 && exceeded {
            return Err(DBError::ValueTooLarge);
        }

        if exceeded {
            file::set_prevlog_readonly(&self.active_file)?;
            self.set_new_active_file();

            self.check_compaction()?;
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

    pub fn set_max_bytes(mut self, bytes: u32) -> Self {
        self.max_bytes = bytes;
        self
    }

    pub fn set_max_logs(mut self, num: u32) -> Self {
        if num < 3 {
            self
        } else {
            self.max_logs = num;
            self
        }
    }

    fn check_compaction(&self) -> Result<(), DBError>{
        let count = file::get_log_file_count(&self.dir)?;

        if count > (self.max_logs as usize) {
            self.compact_log()?;
        }

        Ok(())
    }
}
