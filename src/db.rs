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

#[derive(Debug)]
struct MapValue {
    offset: u32,
    record_size: u32,
    file_id: u32,
}

pub struct DB {
    map: HashMap<String, MapValue>,
    active_file: PathBuf,
    active_file_id: u32,
    tmp_log_file: PathBuf,
    byte_counter: u32,
}

impl DB {
    const TEN_MB: u64 = 10 * 1024 * 1024;

    pub fn open() -> Result<Self, Box<dyn std::error::Error>> {
        let dir = Path::new("logs");

        let mut file_id_counter = 1;
        let mut file_path = dir.join(format!("{:04}.log", file_id_counter));

        if !dir.exists() {
            std::fs::create_dir(dir)?;
        }

        let mut map = HashMap::new();
        let mut byte_counter: u32 = 0;

        loop {
            let mut file_offset: u32 = 0;

            if !file_path.exists() {
                if file_id_counter > 1 {
                    file_id_counter -= 1;
                    file_path = dir.join(format!("{:04}.log", file_id_counter));

                    byte_counter = fs::metadata(&file_path)?.len() as u32;
                }
                println!("Reached end of sequence at {:?}.", file_path);
                break;
            }

            let file = File::open(&file_path)?;
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
                            offset: file_offset,
                            record_size: record_len,
                            file_id: file_path.file_stem().unwrap().to_str().unwrap().parse()?,
                        },
                    ),
                    binarylog::OperationType::Remove => map.remove(str::from_utf8(log.get_key())?),
                };

                let total_len = record_len + std::mem::size_of::<u32>() as u32;

                file_offset += total_len;
            }

            file_id_counter += 1;
            file_path = dir.join(format!("{:04}.log", file_id_counter));
        }

        let tmp_log_file = file_path.with_added_extension("tmp");

        Ok(Self {
            map,
            active_file: file_path,
            active_file_id: file_id_counter,
            tmp_log_file,
            byte_counter,
        })
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), Box<dyn std::error::Error>> {
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

        println!("{:?} | byte_counter: {}", val, self.byte_counter);

        self.map.insert(key, val);

        Ok(())
    }

    pub fn get(&mut self, key: &str) -> Result<binarylog::BinaryLog, Box<dyn std::error::Error>> {
        if let Some(value) = self.map.get(key) {
            let dir = Path::new("logs");
            let cur_path = dir.join(format!("{:04}.log", value.file_id));

            let file = file::open_log(cur_path, false)?;
            let mut reader = BufReader::new(file);

            reader.seek(io::SeekFrom::Start(
                value.offset as u64 + std::mem::size_of::<u32>() as u64,
            ))?;

            println!("{:?} | byte_counter: {}", value, self.byte_counter);

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
        let len_buf: [u8; 4] = serialized[0..4].try_into()?;

        let record_len = u32::from_le_bytes(len_buf);

        let total_len = record_len + std::mem::size_of::<u32>() as u32;

        let append_log_total = self.byte_counter + total_len;

        self.append_log(&serialized, append_log_total)?;

        self.map.remove(&key);

        Ok(())
    }

    // TODO: merge/compact multi segment file
    pub fn compact_log(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.active_file.exists() {
            let metadata = fs::metadata(&self.active_file)?;
            if metadata.len() > DB::TEN_MB {
                {
                    let tmp_log = file::open_log(&self.tmp_log_file, true)?;
                    let mut tmp_writer = BufWriter::new(tmp_log);
                    for (_, val) in self.map.iter() {
                        let file = File::open(&self.active_file)?;
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
                fs::rename(&self.tmp_log_file, &self.active_file)?;
            }
        }

        Ok(())
    }

    fn append_log(&mut self, serialized: &[u8], append_log_total: u32) -> Result<(), Box<dyn std::error::Error>> {
        self.check_log_threshold(append_log_total)?;

        let file = file::open_log(&self.active_file, false)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(serialized)?;
        writer.flush()?;

        Ok(())
    }

    fn check_log_threshold(&mut self, append_log_total: u32) -> Result<(), Box<dyn std::error::Error>> {
        let max_bytes = 70; // 70 bytes only to test multi log files
        println!("log_threshold byte counter: {}", self.byte_counter);
        if append_log_total >= max_bytes {
            println!("exceeded log: {}", self.byte_counter);
            let dir = Path::new("logs");
            self.active_file_id += 1;

            let new_path = dir.join(format!("{:04}.log", self.active_file_id));
            self.active_file = new_path;
            self.byte_counter = 0;
        }

        Ok(())
    }
}
