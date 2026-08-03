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
use crate::binarylog;


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

                let log = binarylog::BinaryLog::deserialize(&record)?;

                match log.get_op_type() {
                    binarylog::OperationType::Set => map.insert(
                        str::from_utf8(log.get_key()).unwrap().to_string(),
                        MapValue {
                            offset: byte_counter,
                            record_size: record_len,
                        },
                    ),
                    binarylog::OperationType::Remove => map.remove(str::from_utf8(log.get_key())?),
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
        let serialized = binarylog::BinaryLog::set(&key, &value);
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

    pub fn get(&self, key: &str) -> Result<binarylog::BinaryLog, Box<dyn std::error::Error>> {
        let file = File::open(&self.log_file)?;
        let mut reader = BufReader::new(file);

        if let Some(value) = self.map.get(key) {
            reader.seek(io::SeekFrom::Start(
                value.offset as u64 + std::mem::size_of::<u32>() as u64,
            ))?;

            let mut record = vec![0u8; value.record_size as usize];

            reader.read_exact(&mut record)?;

            let log = binarylog::BinaryLog::deserialize(&record)?;
            Ok(log)
        } else {
            return Err("Key does not exist.".into());
        }
    }

    pub fn remove(&mut self, key: String) -> Result<(), Box<dyn std::error::Error>> {
        let serialized = binarylog::BinaryLog::remove(&key);

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
}
