use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter};

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

fn save_to_json(
    filename: &str,
    db: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(filename)?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, &db)?;

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let filename = "map.json";

    let mut db: HashMap<String, String> = match File::open(filename) {
        Ok(file) => serde_json::from_reader(BufReader::new(file))?,
        Err(error) => match error.kind() {
            std::io::ErrorKind::NotFound => HashMap::new(),
            _ => return Err(error.into()),
        },
    };

    match args.command {
        Commands::Set { key, value } => {
            db.insert(key, value);
            save_to_json(filename, &db)?;
        }
        Commands::Get { key } => match db.get(&key) {
            Some(value) => println!("{}", value),
            None => println!("key: \"{}\" not found", key),
        },
        Commands::Remove { key } => {
            db.remove(&key);
            save_to_json(filename, &db)?;
        }
    }

    Ok(())
}
