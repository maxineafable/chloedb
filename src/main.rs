use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufRead;
use std::io::Write;
use std::io::{BufReader};
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

fn open_log() -> Result<File, std::io::Error> {
    OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open("log.txt")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut log = open_log()?;

    let reader = BufReader::new(&log);
    let mut map: HashMap<String, String> = HashMap::new();

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

    match args.command {
        Commands::Set { key, value } => {
            let command = format!("SET {} {}", key, value);
            writeln!(log, "{}", command)?;
        }
        Commands::Get { key } => match map.get(&key) {
            Some(value) => println!("{}", value),
            None => println!("key: \"{}\" not found", key),
        },
        Commands::Remove { key } => {
            let command = format!("REMOVE {}", key);
            writeln!(log, "{}", command)?;
        }
    }

    Ok(())
}
