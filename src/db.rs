use std::fs;
use std::io::BufRead;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::{collections::HashMap, fs::File, io::BufReader};
use strum::EnumString;

use crate::file::open_log;

#[derive(EnumString)]
#[strum(serialize_all = "UPPERCASE")]
enum Command {
    Set,
    Remove,
}

pub struct DB {
    map: HashMap<String, String>,
    log_file: PathBuf,
    tmp_log_file: PathBuf,
}

impl DB {
    const TEN_MB: u64 = 10 * 1024 * 1024;

    pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();

        let mut map = HashMap::new();

        if path.exists() {
            let file = File::open(&path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                let mut parts = line.split_whitespace();
                let first = parts.next().ok_or("missing command")?;
                let command = Command::from_str(first)?;
                match command {
                    Command::Set => {
                        let key = parts.next().ok_or("missing key")?;
                        let value = parts.next().ok_or("missing value")?;
                        map.insert(key.to_string(), value.to_string());
                    }
                    Command::Remove => {
                        let key = parts.next().ok_or("missing key")?;
                        map.remove(key);
                    }
                };
            }
        }

        Ok(Self {
            map,
            log_file: path.to_path_buf(),
            tmp_log_file: path.with_added_extension("tmp"),
        })
    }

    pub fn set(&mut self, key: String, value: String) -> Result<(), Box<dyn std::error::Error>> {
        self.append_log(&format!("SET {} {}", key, value))?;
        self.map.insert(key, value);

        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.map.get(key)
    }

    pub fn remove(&mut self, key: String) -> Result<(), Box<dyn std::error::Error>> {
        self.append_log(&format!("REMOVE {}", key))?;
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
                    for (key, val) in self.map.iter() {
                        let command = format!("SET {} {}", key, val);
                        writeln!(tmp_writer, "{}", command)?;
                    }
                    tmp_writer.flush()?;
                }
                fs::rename(&self.tmp_log_file, &self.log_file)?;
            }
        }

        Ok(())
    }

    fn append_log(&self, command: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut log = open_log(&self.log_file, false)?;

        writeln!(log, "{}", command)?;
        log.flush()?;

        Ok(())
    }
}
