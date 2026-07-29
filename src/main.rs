use clap::{Parser, Subcommand};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut db: HashMap<String, String> = HashMap::new();

    match args.command {
        Commands::Set { key, value } => {
            db.insert(key, value);
        }
        Commands::Get { key } => match db.get(&key) {
            Some(value) => println!("{}", value),
            None => println!("Key not found"),
        },
        Commands::Remove { key } => {
            db.remove(&key);
        }
    }

    let file = File::create("map.json")?;
    let writer = BufWriter::new(file);

    serde_json::to_writer_pretty(writer, &db)?;

    Ok(())
}
