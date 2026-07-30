use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;
use strum::EnumString;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Set { key: String, value: String },
    Get { key: String },
    Remove { key: String },
}

#[derive(EnumString)]
#[strum(serialize_all = "UPPERCASE")]
enum Command {
    Set,
    Remove,
}

const TEN_MB: u64 = 10 * 1024 * 1024;

fn open_log(filename: &str, is_temp: bool) -> Result<File, std::io::Error> {
    let mut opts = OpenOptions::new();
    opts.read(true).create(true);

    if is_temp {
        opts.write(true).truncate(true);
    } else {
        opts.append(true);
    }

    opts.open(filename)
}

fn compact_log(map: &HashMap<String, String>) -> Result<(), Box<dyn std::error::Error>> {
    let tmp_filename = "tmp_log.txt";
    let filename = "log.txt";

    if Path::new(filename).exists() {
        let metadata = fs::metadata(filename)?;
        if metadata.len() > TEN_MB {
            {
                let tmp_log = open_log(tmp_filename, true)?;
                let mut tmp_writer = BufWriter::new(tmp_log);
                for (key, val) in map.iter() {
                    let command = format!("SET {} {}", key, val);
                    writeln!(tmp_writer, "{}", command)?;
                }
                tmp_writer.flush()?;
            }
            fs::rename(tmp_filename, filename)?;
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let filename = "log.txt";

    let mut map: HashMap<String, String> = HashMap::new();

    if Path::new(filename).exists() {
        let file = File::open(filename)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            let first = parts[0];
            let command = Command::from_str(first)?;
            match command {
                Command::Set => map.insert(parts[1].to_string(), parts[2].to_string()),
                Command::Remove => map.remove(parts[1]),
            };
        }
    }

    match args.command {
        Commands::Set { key, value } => {
            let mut log = open_log(filename, false)?;

            writeln!(log, "SET {} {}", key, value)?;
            log.flush()?;

            map.insert(key, value);
        }
        Commands::Get { key } => match map.get(&key) {
            Some(value) => println!("{}", value),
            None => println!("key: \"{}\" not found", key),
        },
        Commands::Remove { key } => {
            let mut log = open_log(filename, false)?;

            writeln!(log, "REMOVE {}", key)?;
            log.flush()?;

            map.remove(&key);
        }
    }

    compact_log(&map)?;

    Ok(())
}
